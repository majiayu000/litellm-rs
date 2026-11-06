//! Azure Key Vault secret manager implementation
//!
//! Provides integration with Azure Key Vault for secure secret storage.

#[cfg(feature = "azure-secrets")]
mod implementation {
    use async_trait::async_trait;
    use azure_identity::AzureCliCredential;
    use azure_security_keyvault_secrets::SecretClient;
    use azure_security_keyvault_secrets::models::SetSecretParameters;
    use tracing::{debug, error, warn};

    use crate::core::traits::secret_manager::{
        ListSecretsOptions, ListSecretsResult, SecretError, SecretManager, SecretResult,
    };

    /// Azure Key Vault configuration
    #[derive(Debug, Clone)]
    pub struct AzureSecretsConfig {
        /// Key Vault URL (e.g., https://my-vault.vault.azure.net)
        pub vault_url: String,
        /// Secret name prefix
        pub prefix: Option<String>,
    }

    impl AzureSecretsConfig {
        pub fn new(vault_url: impl Into<String>) -> Self {
            Self {
                vault_url: vault_url.into(),
                prefix: None,
            }
        }

        pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
            self.prefix = Some(prefix.into());
            self
        }

        /// Create from environment variables
        pub fn from_env() -> Option<Self> {
            let vault_url = std::env::var("AZURE_KEYVAULT_URL")
                .or_else(|_| std::env::var("AZURE_VAULT_URL"))
                .ok()?;

            Some(Self {
                vault_url,
                prefix: std::env::var("AZURE_SECRETS_PREFIX").ok(),
            })
        }
    }

    /// Azure Key Vault secret manager
    pub struct AzureSecretManager {
        client: SecretClient,
        config: AzureSecretsConfig,
    }

    impl AzureSecretManager {
        /// Create a new Azure Key Vault client using Azure CLI credentials
        pub fn new(config: AzureSecretsConfig) -> SecretResult<Self> {
            let credential = AzureCliCredential::new(None).map_err(|e| {
                SecretError::auth(format!("Failed to create Azure credential: {}", e))
            })?;

            let client = SecretClient::new(&config.vault_url, credential, None).map_err(|e| {
                SecretError::connection(format!("Failed to create Azure Key Vault client: {}", e))
            })?;

            Ok(Self { client, config })
        }

        /// Create from environment variables
        pub fn from_env() -> SecretResult<Self> {
            let config = AzureSecretsConfig::from_env().ok_or_else(|| {
                SecretError::config("Azure Key Vault URL not found in environment")
            })?;
            Self::new(config)
        }

        /// Get the full secret name with prefix
        fn get_secret_name(&self, name: &str) -> String {
            match &self.config.prefix {
                Some(prefix) => format!("{}{}", prefix, name),
                None => name.to_string(),
            }
        }

        /// Normalize secret name for Azure (replace invalid characters)
        fn normalize_name(&self, name: &str) -> String {
            // Azure Key Vault secret names can only contain alphanumeric characters and dashes
            name.replace(['_', '/', '.'], "-")
        }
    }

    #[async_trait]
    impl SecretManager for AzureSecretManager {
        fn name(&self) -> &'static str {
            "azure"
        }

        async fn read_secret(&self, name: &str) -> SecretResult<Option<String>> {
            let secret_name = self.normalize_name(&self.get_secret_name(name));
            debug!(secret_name = %secret_name, "Reading secret from Azure Key Vault");

            match self.client.get_secret(&secret_name, None).await {
                Ok(response) => {
                    // Response is Response<Secret>, use into_model() to get the Secret
                    let secret = response.into_model().map_err(|e| {
                        SecretError::invalid_format(format!("Failed to parse secret: {}", e))
                    })?;
                    Ok(secret.value)
                }
                Err(err) => {
                    let err_str = err.to_string();
                    if err_str.contains("SecretNotFound") || err_str.contains("404") {
                        debug!(secret_name = %secret_name, "Secret not found");
                        Ok(None)
                    } else if err_str.contains("Forbidden") || err_str.contains("403") {
                        warn!(secret_name = %secret_name, "Access denied to secret");
                        Err(SecretError::access_denied(&secret_name))
                    } else if err_str.contains("Unauthorized") || err_str.contains("401") {
                        error!(secret_name = %secret_name, "Authentication failed");
                        Err(SecretError::auth(err_str))
                    } else {
                        error!(secret_name = %secret_name, error = %err, "Failed to read secret");
                        Err(SecretError::connection(err_str))
                    }
                }
            }
        }

        async fn write_secret(&self, name: &str, value: &str) -> SecretResult<()> {
            let secret_name = self.normalize_name(&self.get_secret_name(name));
            debug!(secret_name = %secret_name, "Writing secret to Azure Key Vault");

            let params = SetSecretParameters {
                value: Some(value.to_string()),
                content_type: None,
                secret_attributes: None,
                tags: None,
            };

            let request_content = params.try_into().map_err(|e: azure_core::Error| {
                SecretError::invalid_format(format!("Failed to create request: {}", e))
            })?;

            match self
                .client
                .set_secret(&secret_name, request_content, None)
                .await
            {
                Ok(_) => {
                    debug!(secret_name = %secret_name, "Secret written successfully");
                    Ok(())
                }
                Err(err) => {
                    let err_str = err.to_string();
                    if err_str.contains("Forbidden") || err_str.contains("403") {
                        Err(SecretError::access_denied(&secret_name))
                    } else if err_str.contains("Unauthorized") || err_str.contains("401") {
                        Err(SecretError::auth(err_str))
                    } else {
                        error!(secret_name = %secret_name, error = %err, "Failed to write secret");
                        Err(SecretError::connection(err_str))
                    }
                }
            }
        }

        async fn delete_secret(&self, name: &str) -> SecretResult<()> {
            let secret_name = self.normalize_name(&self.get_secret_name(name));
            debug!(secret_name = %secret_name, "Deleting secret from Azure Key Vault");

            match self.client.delete_secret(&secret_name, None).await {
                Ok(_) => {
                    debug!(secret_name = %secret_name, "Secret deleted successfully");
                    Ok(())
                }
                Err(err) => {
                    let err_str = err.to_string();
                    if err_str.contains("SecretNotFound") || err_str.contains("404") {
                        // Already deleted
                        Ok(())
                    } else if err_str.contains("Forbidden") || err_str.contains("403") {
                        Err(SecretError::access_denied(&secret_name))
                    } else if err_str.contains("Unauthorized") || err_str.contains("401") {
                        Err(SecretError::auth(err_str))
                    } else {
                        error!(secret_name = %secret_name, error = %err, "Failed to delete secret");
                        Err(SecretError::connection(err_str))
                    }
                }
            }
        }

        async fn list_secrets(
            &self,
            _options: &ListSecretsOptions,
        ) -> SecretResult<ListSecretsResult> {
            // Note: Azure Key Vault list_secret_properties returns a complex Pager type
            // that requires specific handling. For now, return an empty list.
            // Full implementation would require iterating through the pager.
            debug!("Listing secrets from Azure Key Vault (limited support)");

            Ok(ListSecretsResult {
                secrets: vec![],
                next_token: None,
            })
        }
    }

    impl std::fmt::Debug for AzureSecretManager {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AzureSecretManager")
                .field("config", &self.config)
                .finish()
        }
    }

    #[cfg(test)]
    mod security_tests {
        use super::*;
        use azure_core::{
            Bytes,
            credentials::{AccessToken, TokenCredential, TokenRequestOptions},
            http::{
                AsyncRawResponse, Body, HttpClient, Method, Request, StatusCode, Transport,
                headers::{AUTHORIZATION, Headers, WWW_AUTHENTICATE},
            },
            time::{Duration, OffsetDateTime},
        };
        use azure_security_keyvault_secrets::SecretClientOptions;
        use std::sync::{Arc, Mutex};

        #[derive(Debug, Default)]
        struct TestCredential {
            scopes: Mutex<Vec<String>>,
        }

        #[async_trait]
        impl TokenCredential for TestCredential {
            async fn get_token(
                &self,
                scopes: &[&str],
                _options: Option<TokenRequestOptions<'_>>,
            ) -> azure_core::Result<AccessToken> {
                self.scopes
                    .lock()
                    .unwrap()
                    .extend(scopes.iter().map(|scope| scope.to_string()));
                Ok(AccessToken::new(
                    "test-token",
                    OffsetDateTime::now_utc() + Duration::hours(1),
                ))
            }
        }

        #[derive(Debug)]
        struct CapturedRequest {
            method: Method,
            path: String,
            authorization: Option<String>,
            body: Vec<u8>,
        }

        #[derive(Debug)]
        struct ChallengeTransport {
            challenge: &'static str,
            requests: Mutex<Vec<CapturedRequest>>,
        }

        #[async_trait]
        impl HttpClient for ChallengeTransport {
            async fn execute_request(
                &self,
                request: &Request,
            ) -> azure_core::Result<AsyncRawResponse> {
                let authorization = request.headers().get_str(&AUTHORIZATION).ok();
                let Body::Bytes(body) = request.body() else {
                    panic!("secret operations should send a buffered JSON body");
                };
                self.requests.lock().unwrap().push(CapturedRequest {
                    method: request.method(),
                    path: request.url().path().to_string(),
                    authorization: authorization.map(str::to_owned),
                    body: body.to_vec(),
                });
                let mut headers = Headers::new();
                if authorization.is_none() {
                    headers.insert(WWW_AUTHENTICATE, self.challenge);
                    Ok(AsyncRawResponse::from_bytes(
                        StatusCode::Unauthorized,
                        headers,
                        Bytes::new(),
                    ))
                } else {
                    Ok(AsyncRawResponse::from_bytes(
                        StatusCode::Ok,
                        headers,
                        Bytes::from_static(br#"{"value":"stored-value"}"#),
                    ))
                }
            }
        }

        fn test_manager(
            challenge: &'static str,
        ) -> (
            AzureSecretManager,
            Arc<TestCredential>,
            Arc<ChallengeTransport>,
        ) {
            let credential = Arc::new(TestCredential::default());
            let transport = Arc::new(ChallengeTransport {
                challenge,
                requests: Mutex::new(Vec::new()),
            });
            let config =
                AzureSecretsConfig::new("https://test-vault.vault.azure.net").prefix("prod/");
            let mut options = SecretClientOptions::default();
            options.client_options.transport = Some(Transport::new(transport.clone()));
            let client = SecretClient::new(&config.vault_url, credential.clone(), Some(options))
                .expect("test client should build");
            (AzureSecretManager { client, config }, credential, transport)
        }

        #[tokio::test]
        async fn rejected_challenges_do_not_leak_tokens_or_secret_bodies_on_client_reuse() {
            for challenge in [
                r#"Bearer resource="https://attacker.example""#,
                r#"Bearer scope="https://attacker.example/.default""#,
            ] {
                let (manager, credential, transport) = test_manager(challenge);
                for _ in 0..2 {
                    manager
                        .write_secret("api_key", "must-not-leak")
                        .await
                        .expect_err("every mismatched challenge must be rejected");
                }
                assert!(credential.scopes.lock().unwrap().is_empty());
                let requests = transport.requests.lock().unwrap();
                assert_eq!(requests.len(), 2);
                for request in requests.iter() {
                    assert!(request.authorization.is_none());
                    assert!(request.body.is_empty());
                }
            }
        }

        #[tokio::test]
        async fn valid_challenge_preserves_secret_read_write_and_delete() {
            let (manager, credential, transport) =
                test_manager(r#"Bearer resource="https://vault.azure.net""#);
            assert_eq!(
                manager.read_secret("api_key").await.unwrap(),
                Some("stored-value".to_string()),
            );
            manager.write_secret("api_key", "new-value").await.unwrap();
            manager.delete_secret("api_key").await.unwrap();

            assert_eq!(
                *credential.scopes.lock().unwrap(),
                ["https://vault.azure.net/.default"],
            );
            let requests = transport.requests.lock().unwrap();
            assert_eq!(requests.len(), 4);
            assert!(requests[0].authorization.is_none());
            assert_eq!(requests[1].method, Method::Get);
            assert_eq!(requests[2].method, Method::Put);
            assert_eq!(requests[3].method, Method::Delete);
            // An empty version selects the latest secret on GET.
            assert_eq!(requests[1].path, "/secrets/prod-api-key/");
            assert_eq!(requests[2].path, "/secrets/prod-api-key");
            assert_eq!(requests[3].path, "/secrets/prod-api-key");
            for request in &requests[1..] {
                assert_eq!(request.authorization.as_deref(), Some("Bearer test-token"));
            }
            let body: serde_json::Value = serde_json::from_slice(&requests[2].body).unwrap();
            assert_eq!(body["value"], "new-value");
        }
    }
}

#[cfg(feature = "azure-secrets")]
pub use implementation::*;

// Stub implementation when feature is disabled
#[cfg(not(feature = "azure-secrets"))]
mod stub {
    use crate::core::traits::secret_manager::SecretError;

    /// Azure Key Vault configuration (stub)
    #[derive(Debug, Clone)]
    pub struct AzureSecretsConfig {
        pub vault_url: String,
    }

    impl AzureSecretsConfig {
        pub fn new(vault_url: impl Into<String>) -> Self {
            Self {
                vault_url: vault_url.into(),
            }
        }

        pub fn from_env() -> Option<Self> {
            None
        }
    }

    /// Azure Key Vault secret manager (stub)
    #[derive(Debug)]
    pub struct AzureSecretManager;

    impl AzureSecretManager {
        pub fn new(_config: AzureSecretsConfig) -> Result<Self, SecretError> {
            Err(SecretError::config(
                "Azure Key Vault support not enabled. Enable the 'azure-secrets' feature.",
            ))
        }

        pub fn from_env() -> Result<Self, SecretError> {
            Err(SecretError::config(
                "Azure Key Vault support not enabled. Enable the 'azure-secrets' feature.",
            ))
        }
    }
}

#[cfg(not(feature = "azure-secrets"))]
pub use stub::*;

#[cfg(all(test, feature = "azure-secrets"))]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = AzureSecretsConfig::new("https://my-vault.vault.azure.net").prefix("prod/");

        assert_eq!(config.vault_url, "https://my-vault.vault.azure.net");
        assert_eq!(config.prefix, Some("prod/".to_string()));
    }
}
