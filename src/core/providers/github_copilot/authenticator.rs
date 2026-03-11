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
#[derive(Debug, Clone)]
pub struct CopilotAuthenticator {
    /// Token directory path
    token_dir: PathBuf,
    /// Access token file path
    access_token_path: PathBuf,
    /// API key file path
    api_key_path: PathBuf,
}

impl CopilotAuthenticator {
    /// Create a new authenticator from configuration
    pub fn new(config: &GitHubCopilotConfig) -> Self {
        let token_dir = PathBuf::from(config.get_token_dir());
        let access_token_path = token_dir.join(config.get_access_token_file());
        let api_key_path = token_dir.join(config.get_api_key_file());

        Self {
            token_dir,
            access_token_path,
            api_key_path,
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
        // Try to read from cache first
        if let Ok(token) = fs::read_to_string(&self.access_token_path) {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
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
                    if let Err(e) = fs::write(&self.access_token_path, &token) {
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
        // Try to read from cache first
        if let Ok(content) = fs::read_to_string(&self.api_key_path)
            && let Ok(api_key_info) = serde_json::from_str::<ApiKeyInfo>(&content)
        {
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
        if let Ok(content) = fs::read_to_string(&self.api_key_path)
            && let Ok(api_key_info) = serde_json::from_str::<ApiKeyInfo>(&content)
        {
            return api_key_info.endpoints.api;
        }
        None
    }

    /// Refresh the API key using the access token
    async fn refresh_api_key(&self) -> Result<String, GitHubCopilotError> {
        let access_token = self.get_access_token().await?;
        let headers = self.get_github_headers(Some(&access_token));

        let client = reqwest::Client::new();

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
            if let Ok(json) = serde_json::to_string(&api_key_info)
                && let Err(e) = fs::write(&self.api_key_path, json)
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
        let client = reqwest::Client::new();
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
        headers.insert("accept", "application/json".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("editor-version", "vscode/1.85.1".parse().unwrap());
        headers.insert("editor-plugin-version", "copilot/1.155.0".parse().unwrap());
        headers.insert("user-agent", "GithubCopilot/1.155.0".parse().unwrap());
        headers.insert("accept-encoding", "gzip,deflate,br".parse().unwrap());

        if let Some(token) = access_token {
            headers.insert("authorization", format!("token {}", token).parse().unwrap());
        }

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
