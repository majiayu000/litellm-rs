//! GitHub Copilot OAuth Device Flow Authenticator
//!
//! Handles GitHub Copilot authentication using the OAuth Device Flow.
//! Manages access tokens and API keys with automatic refresh.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use super::config::GitHubCopilotConfig;
use super::error::GitHubCopilotError;
use crate::core::providers::unified_provider::ProviderError;

/// GitHub OAuth client ID for Copilot
const GITHUB_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

/// GitHub device code URL
const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";

/// GitHub access token URL
const GITHUB_ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

/// GitHub Copilot API key URL
const GITHUB_API_KEY_URL: &str = "https://api.github.com/copilot_internal/v2/token";

/// API key information stored in the cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    /// The API token
    pub token: String,
    /// Expiration timestamp (Unix timestamp)
    pub expires_at: u64,
    /// API endpoints
    #[serde(default)]
    pub endpoints: Endpoints,
}

/// API endpoints returned by GitHub
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Endpoints {
    /// API endpoint URL
    pub api: Option<String>,
}

/// Device code response from GitHub
#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    interval: u64,
}

/// Access token response from GitHub
#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

/// GitHub Copilot OAuth authenticator
#[derive(Clone)]
pub struct CopilotAuthenticator {
    /// Token directory path
    token_dir: PathBuf,
    /// Access token file path
    access_token_path: PathBuf,
    /// API key file path
    api_key_path: PathBuf,
    access_token: Arc<RwLock<Option<String>>>,
    api_key: Arc<RwLock<Option<ApiKeyInfo>>>,
    credential_files: Arc<RwLock<[Option<Vec<u8>>; 2]>>,
    credential_identity: [u8; 32],
}

impl std::fmt::Debug for CopilotAuthenticator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CopilotAuthenticator")
            .field("credentials", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl CopilotAuthenticator {
    /// Create a new authenticator from configuration
    pub fn new(config: &GitHubCopilotConfig) -> Self {
        let token_dir = PathBuf::from(config.get_token_dir());
        let access_token_path = token_dir.join(config.get_access_token_file());
        let api_key_path = token_dir.join(config.get_api_key_file());
        // Capture the same file-backed authority used by this resource. External
        // account rotation takes effect on reconstruction, not on an old lease.
        let access_file = fs::read(&access_token_path).ok();
        let api_key_file = fs::read(&api_key_path).ok();
        let access_token = access_file
            .as_ref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());
        let api_key = api_key_file
            .as_ref()
            .and_then(|bytes| serde_json::from_slice::<ApiKeyInfo>(bytes).ok());
        let mut identity = serde_json::json!({
            "token_dir": token_dir,
            "access_token_path": access_token_path,
            "api_key_path": api_key_path,
            "access_token": access_token,
            "api_key": api_key,
        });
        identity.sort_all_objects();
        let credential_identity = Sha256::digest(identity.to_string().as_bytes()).into();

        Self {
            token_dir,
            access_token_path,
            api_key_path,
            access_token: Arc::new(RwLock::new(access_token)),
            api_key: Arc::new(RwLock::new(api_key)),
            credential_files: Arc::new(RwLock::new([access_file, api_key_file])),
            credential_identity,
        }
    }

    pub(super) fn credential_resource_identity(&self) -> [u8; 32] {
        self.credential_identity
    }

    fn persist_credential(&self, path: &Path, contents: &[u8]) -> std::io::Result<()> {
        let mut files = self.credential_files.write();
        let paths = [&self.access_token_path, &self.api_key_path];
        for (captured_path, captured) in paths.iter().zip(files.iter()) {
            let changed = match fs::read(captured_path) {
                Ok(bytes) => captured.as_ref() != Some(&bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => captured.is_some(),
                Err(error) => return Err(error),
            };
            if changed {
                // Detected external rotation owns these files. Keep this resource's
                // refresh in memory; this check is not an inter-process atomic write.
                debug!("Copilot credential files changed; keeping refreshed credentials in memory");
                return Ok(());
            }
        }
        fs::write(path, contents)?;
        for (captured_path, captured) in paths.iter().zip(files.iter_mut()) {
            if captured_path.as_path() == path {
                *captured = Some(contents.to_vec());
            }
        }
        Ok(())
    }

    /// Ensure the token directory exists
    fn ensure_token_dir(&self) -> Result<(), GitHubCopilotError> {
        if !self.token_dir.exists() {
            fs::create_dir_all(&self.token_dir).map_err(|e| {
                ProviderError::configuration(
                    "github_copilot",
                    format!("Failed to create token directory: {}", e),
                )
            })?;
        }
        Ok(())
    }

    /// Get the access token, performing device flow authentication if needed
    pub async fn get_access_token(&self) -> Result<String, GitHubCopilotError> {
        if let Some(token) = self.access_token.read().clone() {
            return Ok(token);
        }

        // Need to perform device flow authentication
        warn!("No cached access token found, need to authenticate");

        // Retry up to 3 times
        for attempt in 1..=3 {
            debug!("Access token acquisition attempt {}/3", attempt);
            match self.perform_device_flow().await {
                Ok(token) => {
                    // Save to cache
                    self.ensure_token_dir()?;
                    *self.access_token.write() = Some(token.clone());
                    if let Err(e) =
                        self.persist_credential(&self.access_token_path, token.as_bytes())
                    {
                        warn!("Failed to cache access token: {}", e);
                    }
                    return Ok(token);
                }
                Err(e) => {
                    warn!("Device flow attempt {} failed: {}", attempt, e);
                    if attempt == 3 {
                        return Err(ProviderError::authentication(
                            "github_copilot",
                            "Access token error: Failed to get access token after 3 attempts",
                        ));
                    }
                }
            }
        }

        Err(ProviderError::authentication(
            "github_copilot",
            "Access token error: Failed to get access token",
        ))
    }

    /// Get the API key, refreshing if needed
    pub async fn get_api_key(&self) -> Result<String, GitHubCopilotError> {
        if let Some(api_key_info) = self.api_key.read().clone() {
            // Check if not expired
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();

            if api_key_info.expires_at > now {
                return Ok(api_key_info.token);
            }
            debug!("API key expired, refreshing...");
        }

        // Need to refresh
        self.refresh_api_key().await
    }

    /// Get the API base URL from cached API key info
    pub fn get_api_base(&self) -> Option<String> {
        self.api_key
            .read()
            .as_ref()
            .and_then(|info| info.endpoints.api.clone())
    }

    /// Refresh the API key using the access token
    async fn refresh_api_key(&self) -> Result<String, GitHubCopilotError> {
        let access_token = self.get_access_token().await?;
        let headers = self.get_github_headers(Some(&access_token));

        let client = crate::core::http::outbound::default_outbound_client().clone();

        for attempt in 1..=3 {
            let response = client
                .get(GITHUB_API_KEY_URL)
                .headers(headers.clone())
                .send()
                .await
                .map_err(|e| {
                    ProviderError::authentication(
                        "github_copilot",
                        format!("Refresh error: HTTP error: {}", e),
                    )
                })?;

            if !response.status().is_success() {
                warn!(
                    "API key refresh attempt {}/3 failed with status: {}",
                    attempt,
                    response.status()
                );
                if attempt == 3 {
                    return Err(ProviderError::authentication(
                        "github_copilot",
                        "Refresh error: Failed to refresh API key after 3 attempts",
                    ));
                }
                continue;
            }

            let api_key_info: ApiKeyInfo = response.json().await.map_err(|e| {
                ProviderError::authentication(
                    "github_copilot",
                    format!("Refresh error: Failed to parse response: {}", e),
                )
            })?;

            // Save to cache
            self.ensure_token_dir()?;
            *self.api_key.write() = Some(api_key_info.clone());
            if let Ok(json) = serde_json::to_string(&api_key_info)
                && let Err(e) = self.persist_credential(&self.api_key_path, json.as_bytes())
            {
                warn!("Failed to cache API key: {}", e);
            }

            return Ok(api_key_info.token);
        }

        Err(ProviderError::authentication(
            "github_copilot",
            "Refresh error: Failed to refresh API key",
        ))
    }

    /// Perform the OAuth device flow
    async fn perform_device_flow(&self) -> Result<String, GitHubCopilotError> {
        let client = crate::core::http::outbound::default_outbound_client().clone();
        let headers = self.get_github_headers(None);

        // Step 1: Get device code
        let response = client
            .post(GITHUB_DEVICE_CODE_URL)
            .headers(headers.clone())
            .json(&serde_json::json!({
                "client_id": GITHUB_CLIENT_ID,
                "scope": "read:user"
            }))
            .send()
            .await
            .map_err(|e| {
                ProviderError::authentication(
                    "github_copilot",
                    format!("Device code error: HTTP error: {}", e),
                )
            })?;

        if !response.status().is_success() {
            return Err(ProviderError::authentication(
                "github_copilot",
                format!(
                    "Device code error: Failed to get device code: {}",
                    response.status()
                ),
            ));
        }

        let device_code_response: DeviceCodeResponse = response.json().await.map_err(|e| {
            ProviderError::authentication(
                "github_copilot",
                format!("Device code error: Failed to parse response: {}", e),
            )
        })?;

        // Print user instructions
        println!(
            "\nPlease visit {} and enter code {} to authenticate.\n",
            device_code_response.verification_uri, device_code_response.user_code
        );

        // Step 2: Poll for access token
        let interval = if device_code_response.interval > 0 {
            device_code_response.interval
        } else {
            5
        };

        let max_attempts = 60 / interval as usize; // 1 minute max

        for _attempt in 0..max_attempts {
            tokio::time::sleep(Duration::from_secs(interval)).await;

            let response = client
                .post(GITHUB_ACCESS_TOKEN_URL)
                .headers(headers.clone())
                .json(&serde_json::json!({
                    "client_id": GITHUB_CLIENT_ID,
                    "device_code": device_code_response.device_code,
                    "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
                }))
                .send()
                .await
                .map_err(|e| {
                    ProviderError::authentication(
                        "github_copilot",
                        format!("Access token error: HTTP error: {}", e),
                    )
                })?;

            let token_response: AccessTokenResponse = response.json().await.map_err(|e| {
                ProviderError::authentication(
                    "github_copilot",
                    format!("Access token error: Failed to parse response: {}", e),
                )
            })?;

            if let Some(access_token) = token_response.access_token {
                debug!("Authentication successful!");
                return Ok(access_token);
            }

            if let Some(error) = &token_response.error
                && error != "authorization_pending"
            {
                return Err(ProviderError::authentication(
                    "github_copilot",
                    format!("Access token error: OAuth error: {}", error),
                ));
            }
        }

        Err(ProviderError::authentication(
            "github_copilot",
            "Access token error: Timed out waiting for user authorization",
        ))
    }

    /// Get standard GitHub headers
    fn get_github_headers(&self, access_token: Option<&str>) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "accept",
            "application/json"
                .parse()
                .expect("static header value 'application/json' is always valid"),
        );
        headers.insert(
            "content-type",
            "application/json"
                .parse()
                .expect("static header value 'application/json' is always valid"),
        );
        headers.insert(
            "editor-version",
            "vscode/1.85.1"
                .parse()
                .expect("static header value 'vscode/1.85.1' is always valid"),
        );
        headers.insert(
            "editor-plugin-version",
            "copilot/1.155.0"
                .parse()
                .expect("static header value 'copilot/1.155.0' is always valid"),
        );
        headers.insert(
            "user-agent",
            "GithubCopilot/1.155.0"
                .parse()
                .expect("static header value 'GithubCopilot/1.155.0' is always valid"),
        );
        headers.insert(
            "accept-encoding",
            "gzip,deflate,br"
                .parse()
                .expect("static header value 'gzip,deflate,br' is always valid"),
        );

        if let Some(token) = access_token {
            if let Ok(value) = format!("token {}", token).parse() {
                headers.insert("authorization", value);
            } else {
                tracing::warn!(
                    "GitHub Copilot access token contains invalid header characters, skipping authorization header"
                );
            }
        }

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn captured_credentials_survive_file_rotation_and_clone() {
        let directory = tempfile::tempdir().unwrap();
        let access_path = directory.path().join("access-token");
        let key_path = directory.path().join("api-key.json");
        let write = |suffix: &str| {
            fs::write(&access_path, format!("  fixture-access-{suffix}\n")).unwrap();
            fs::write(
                &key_path,
                serde_json::json!({
                    "token": format!("fixture-key-{suffix}"), "expires_at": u64::MAX,
                    "endpoints": {"api": format!("https://{suffix}.fixture.invalid")}
                })
                .to_string(),
            )
            .unwrap();
        };
        write("a");
        let config = GitHubCopilotConfig {
            token_dir: Some(directory.path().to_str().unwrap().into()),
            access_token_file: Some("access-token".into()),
            api_key_file: Some("api-key.json".into()),
            ..Default::default()
        };
        let original = CopilotAuthenticator::new(&config);
        let replica = CopilotAuthenticator::new(&config);
        let cloned = original.clone();
        assert_eq!(
            original.credential_resource_identity(),
            replica.credential_resource_identity()
        );
        write("b");
        for captured in [&original, &cloned] {
            assert_eq!(
                captured.get_access_token().await.unwrap(),
                "fixture-access-a"
            );
            assert_eq!(captured.get_api_key().await.unwrap(), "fixture-key-a");
            assert_eq!(
                captured.get_api_base().as_deref(),
                Some("https://a.fixture.invalid")
            );
        }
        let replacement = CopilotAuthenticator::new(&config);
        assert_ne!(
            original.credential_resource_identity(),
            replacement.credential_resource_identity()
        );
        assert_eq!(
            replacement.get_access_token().await.unwrap(),
            "fixture-access-b"
        );
        assert_eq!(replacement.get_api_key().await.unwrap(), "fixture-key-b");
        // Normal refresh changes this resource's cache, not its construction identity.
        let identity = original.credential_resource_identity();
        original.api_key.write().as_mut().unwrap().token = "fixture-refreshed-key".into();
        assert_eq!(cloned.get_api_key().await.unwrap(), "fixture-refreshed-key");
        assert_eq!(original.credential_resource_identity(), identity);
        assert!(!format!("{original:?}").contains("fixture-"));
    }

    #[tokio::test]
    async fn captured_refresh_preserves_rotated_files_and_tracks_owned_writes() {
        for rotation in ["none", "access", "api", "both"] {
            let directory = tempfile::tempdir().unwrap();
            let access_path = directory.path().join("access-token");
            let key_path = directory.path().join("api-key.json");
            let key = |suffix: &str| ApiKeyInfo {
                token: format!("fixture-key-{suffix}"),
                expires_at: u64::MAX,
                endpoints: Endpoints {
                    api: Some(format!("https://{suffix}.fixture.invalid")),
                },
            };
            fs::write(&access_path, "fixture-access-a").unwrap();
            fs::write(&key_path, serde_json::to_vec(&key("a")).unwrap()).unwrap();
            let config = GitHubCopilotConfig {
                token_dir: Some(directory.path().to_str().unwrap().into()),
                access_token_file: Some("access-token".into()),
                api_key_file: Some("api-key.json".into()),
                ..Default::default()
            };
            let captured = CopilotAuthenticator::new(&config);
            let cloned = captured.clone();
            let identity = captured.credential_resource_identity();
            if matches!(rotation, "access" | "both") {
                fs::write(&access_path, "fixture-access-b").unwrap();
            }
            if matches!(rotation, "api" | "both") {
                fs::write(&key_path, serde_json::to_vec(&key("b")).unwrap()).unwrap();
            }
            let access_before = fs::read(&access_path).unwrap();
            let key_before = fs::read(&key_path).unwrap();
            for suffix in ["refreshed-1", "refreshed-2"] {
                let refreshed = key(suffix);
                // Exercise the production refresh's persistence boundary after
                // its directory check and construction-owned cache update.
                captured.ensure_token_dir().unwrap();
                *captured.api_key.write() = Some(refreshed.clone());
                cloned
                    .persist_credential(&key_path, &serde_json::to_vec(&refreshed).unwrap())
                    .unwrap();
                assert_eq!(cloned.get_access_token().await.unwrap(), "fixture-access-a");
                assert_eq!(cloned.get_api_key().await.unwrap(), refreshed.token);
                assert_eq!(cloned.get_api_base(), refreshed.endpoints.api);
                assert_eq!(cloned.credential_resource_identity(), identity);
                assert_eq!(fs::read(&access_path).unwrap(), access_before);
                if rotation == "none" {
                    assert_eq!(
                        fs::read(&key_path).unwrap(),
                        serde_json::to_vec(&refreshed).unwrap()
                    );
                } else {
                    assert_eq!(fs::read(&key_path).unwrap(), key_before, "{rotation}");
                }
            }
            let replacement = CopilotAuthenticator::new(&config);
            assert_ne!(replacement.credential_resource_identity(), identity);
            let expected_access = if matches!(rotation, "access" | "both") {
                "fixture-access-b"
            } else {
                "fixture-access-a"
            };
            let expected_key = match rotation {
                "none" => "fixture-key-refreshed-2",
                "access" => "fixture-key-a",
                _ => "fixture-key-b",
            };
            assert_eq!(
                replacement.get_access_token().await.unwrap(),
                expected_access
            );
            assert_eq!(replacement.get_api_key().await.unwrap(), expected_key);
        }
    }

    #[test]
    fn captured_absence_does_not_treat_read_errors_as_owned_files() {
        let directory = tempfile::tempdir().unwrap();
        let config = GitHubCopilotConfig {
            token_dir: Some(directory.path().to_str().unwrap().into()),
            access_token_file: Some("access-token".into()),
            api_key_file: Some("api-key.json".into()),
            ..Default::default()
        };
        let captured = CopilotAuthenticator::new(&config);
        captured
            .persist_credential(&captured.api_key_path, b"fixture-initial")
            .unwrap();
        fs::create_dir(&captured.access_token_path).unwrap();
        assert!(
            captured
                .persist_credential(&captured.api_key_path, b"fixture-replacement")
                .is_err()
        );
        assert_eq!(
            fs::read(&captured.api_key_path).unwrap(),
            b"fixture-initial"
        );
    }

    #[test]
    fn absent_or_invalid_cache_preserves_lazy_authentication() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("access-token"), " \n").unwrap();
        fs::write(
            directory.path().join("api-key.json"),
            "invalid fixture JSON",
        )
        .unwrap();
        let config = GitHubCopilotConfig {
            token_dir: Some(directory.path().to_str().unwrap().into()),
            access_token_file: Some("access-token".into()),
            api_key_file: Some("api-key.json".into()),
            ..Default::default()
        };
        let auth = CopilotAuthenticator::new(&config);
        assert!(auth.access_token.read().is_none());
        assert!(auth.api_key.read().is_none());
        assert!(auth.get_api_base().is_none());
    }

    #[test]
    fn test_authenticator_creation() {
        let config = GitHubCopilotConfig::default();
        let auth = CopilotAuthenticator::new(&config);

        assert!(auth.token_dir.to_string_lossy().contains("github_copilot"));
    }

    #[test]
    fn test_api_key_info_serialization() {
        let info = ApiKeyInfo {
            token: "test-token".to_string(),
            expires_at: 1234567890,
            endpoints: Endpoints {
                api: Some("https://api.example.com".to_string()),
            },
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-token"));
        assert!(json.contains("1234567890"));

        let deserialized: ApiKeyInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.token, "test-token");
        assert_eq!(deserialized.expires_at, 1234567890);
    }

    #[test]
    fn test_api_key_info_deserialization_with_defaults() {
        let json = r#"{"token": "test", "expires_at": 123}"#;
        let info: ApiKeyInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.token, "test");
        assert!(info.endpoints.api.is_none());
    }

    #[test]
    fn test_get_github_headers() {
        let config = GitHubCopilotConfig::default();
        let auth = CopilotAuthenticator::new(&config);

        let headers = auth.get_github_headers(None);
        assert!(headers.get("accept").is_some());
        assert!(headers.get("user-agent").is_some());
        assert!(headers.get("authorization").is_none());

        let headers = auth.get_github_headers(Some("test-token"));
        assert!(headers.get("authorization").is_some());
    }
}
