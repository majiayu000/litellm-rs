//! GitHub Copilot OAuth Device Flow Authenticator
//!
//! Handles GitHub Copilot authentication using the OAuth Device Flow.
//! Manages access tokens and API keys with automatic refresh.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::config::GitHubCopilotConfig;
use super::error::GitHubCopilotError;
use crate::core::providers::unified_provider::ProviderError;

#[path = "credential_snapshot.rs"]
mod credential_snapshot;
use credential_snapshot::{CapturedCredentials, CredentialUpdate};

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
    /// Construction-owned credentials shared by this authenticator and its clones.
    captured: Option<std::sync::Arc<CapturedCredentials>>,
    #[cfg(test)]
    api_key_url: Option<String>,
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
        Self::capture(config).0
    }

    // Resolve paths without recursively constructing another captured resource.
    fn from_paths(config: &GitHubCopilotConfig) -> Self {
        let token_dir = PathBuf::from(config.get_token_dir());
        let access_token_path = token_dir.join(config.get_access_token_file());
        let api_key_path = token_dir.join(config.get_api_key_file());

        Self {
            token_dir,
            access_token_path,
            api_key_path,
            captured: None,
            #[cfg(test)]
            api_key_url: None,
        }
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
        let _acquisition = match &self.captured {
            Some(captured) => Some(captured.access_token_gate.lock().await),
            None => None,
        };
        // Try to read from cache first
        if let Some(token) = self.cached_access_token() {
            return Ok(token);
        }
        if !self.device_flow_allowed() {
            return Err(ProviderError::authentication(
                "github_copilot",
                "No access token was captured for API key refresh; configure credentials and reload the provider",
            ));
        }

        // Need to perform device flow authentication
        warn!("No cached access token found, need to authenticate");

        // Retry up to 3 times
        for attempt in 1..=3 {
            debug!("Access token acquisition attempt {}/3", attempt);
            match self.perform_device_flow().await {
                Ok(token) => {
                    // Save to cache
                    self.cache_credential(CredentialUpdate::AccessToken(&token))?;
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
        self.get_api_key_info().await.map(|info| info.token)
    }

    /// Return the token and endpoint from the same credential snapshot.
    pub(crate) async fn get_api_key_info(&self) -> Result<ApiKeyInfo, GitHubCopilotError> {
        let _refresh = match &self.captured {
            Some(captured) => Some(captured.api_key_gate.lock().await),
            None => None,
        };
        // Try to read from cache first
        if let Some(api_key_info) = self.cached_api_key() {
            // Check if not expired
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();

            if api_key_info.expires_at > now {
                return Ok(api_key_info);
            }
            debug!("API key expired, refreshing...");
        }

        // Need to refresh
        self.refresh_api_key().await
    }

    /// Get the API base URL from cached API key info
    pub fn get_api_base(&self) -> Option<String> {
        self.cached_api_key().and_then(|info| info.endpoints.api)
    }

    /// Refresh the API key using the access token
    async fn refresh_api_key(&self) -> Result<ApiKeyInfo, GitHubCopilotError> {
        let access_token = self.get_access_token().await?;
        let headers = self.get_github_headers(Some(&access_token));

        let client = crate::core::http::outbound::default_outbound_client().clone();
        #[cfg(test)]
        let api_key_url = self.api_key_url.as_deref().unwrap_or(GITHUB_API_KEY_URL);
        #[cfg(not(test))]
        let api_key_url = GITHUB_API_KEY_URL;

        for attempt in 1..=3 {
            let response = client
                .get(api_key_url)
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
            self.cache_credential(CredentialUpdate::ApiKey(&api_key_info))?;

            return Ok(api_key_info);
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
        let mut refreshed = original.cached_api_key().unwrap();
        refreshed.token = "fixture-refreshed-key".into();
        original
            .cache_credential(CredentialUpdate::ApiKey(&refreshed))
            .unwrap();
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
                captured
                    .cache_credential(CredentialUpdate::ApiKey(&refreshed))
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
        assert!(auth.cached_access_token().is_none());
        assert!(auth.cached_api_key().is_none());
        assert!(auth.get_api_base().is_none());
    }

    #[test]
    fn test_authenticator_creation() {
        let _environment = crate::core::providers::factory::CONSTRUCTION_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
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
