//! Vertex AI Authentication
//!
//! Supports multiple authentication methods:
//! - Service Account JSON
//! - Workload Identity Federation
//! - Application Default Credentials (ADC)
//! - Access Token

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Vertex AI Authentication credentials
#[derive(Debug, Clone)]
pub enum VertexCredentials {
    /// Service Account JSON key
    ServiceAccount(ServiceAccountKey),

    /// Workload Identity Federation
    WorkloadIdentity(WorkloadIdentityConfig),

    /// Application Default Credentials
    ApplicationDefault,

    /// Direct access token
    AccessToken(String),

    /// Authorized User (from gcloud auth)
    AuthorizedUser(AuthorizedUserCredentials),
}

/// Service Account key structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceAccountKey {
    #[serde(rename = "type")]
    pub key_type: String,
    pub project_id: String,
    pub private_key_id: String,
    pub private_key: String,
    pub client_email: String,
    pub client_id: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub auth_provider_x509_cert_url: String,
    pub client_x509_cert_url: String,
}

/// Workload Identity Federation configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkloadIdentityConfig {
    #[serde(rename = "type")]
    pub config_type: String,
    pub audience: String,
    pub subject_token_type: String,
    pub service_account_impersonation_url: Option<String>,
    pub token_url: String,
    pub credential_source: CredentialSource,
    pub quota_project_id: Option<String>,
}

/// Credential source for Workload Identity
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CredentialSource {
    pub file: Option<String>,
    pub url: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub environment_id: Option<String>,
    pub regional_cred_verification_url: Option<String>,
}

/// Authorized User credentials (from gcloud)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthorizedUserCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
    #[serde(rename = "type")]
    pub cred_type: String,
}

/// OAuth2 Token with expiration
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub token_type: String,
}

#[derive(Debug, Error)]
pub enum VertexAuthError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("Failed to read credentials file: {0}")]
    ReadCredentialsFile(std::io::Error),
    #[error("Failed to read subject token file: {0}")]
    ReadSubjectTokenFile(std::io::Error),
    #[error("Failed to fetch subject token: {0}")]
    FetchSubjectToken(reqwest::Error),
    #[error("Failed to get project ID from metadata: {0}")]
    GetProjectIdFromMetadata(reqwest::Error),
    #[error("Unknown credential type")]
    UnknownCredentialType,
    #[error("Unsupported environment ID: {0}")]
    UnsupportedEnvironmentId(String),
    #[error("No credential source specified")]
    MissingCredentialSource,
    #[error("AWS token retrieval not yet implemented")]
    AwsTokenNotImplemented,
    #[error("Unable to get ADC token. Please run 'gcloud auth application-default login'")]
    AdcTokenUnavailable,
    #[error("No project ID in workload identity config")]
    MissingProjectId,
}

type Result<T> = std::result::Result<T, VertexAuthError>;

impl AccessToken {
    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at - Duration::minutes(5) // 5 min buffer
    }
}

/// Vertex AI Authentication handler
#[derive(Debug, Clone)]
pub struct VertexAuth {
    credentials: VertexCredentials,
    token_cache: Arc<RwLock<Option<AccessToken>>>,
    http_client: reqwest::Client,
}

impl VertexAuth {
    /// Create new authentication handler
    pub fn new(credentials: VertexCredentials) -> Self {
        Self {
            credentials,
            token_cache: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::new(),
        }
    }

    /// Load credentials from environment or file
    pub async fn from_env() -> Result<Self> {
        // Try GOOGLE_APPLICATION_CREDENTIALS first
        if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            let credentials = Self::load_credentials_from_file(&path).await?;
            return Ok(Self::new(credentials));
        }

        // Try VERTEX_AI_CREDENTIALS
        if let Ok(json_str) = std::env::var("VERTEX_AI_CREDENTIALS") {
            let credentials = Self::parse_credentials(&json_str)?;
            return Ok(Self::new(credentials));
        }

        // Fall back to Application Default Credentials
        Ok(Self::new(VertexCredentials::ApplicationDefault))
    }

    /// Load credentials from a JSON file
    pub async fn load_credentials_from_file(path: &str) -> Result<VertexCredentials> {
        let contents = tokio::fs::read_to_string(path)
            .await
            .map_err(VertexAuthError::ReadCredentialsFile)?;
        Self::parse_credentials(&contents)
    }

    /// Parse credentials from JSON string
    pub fn parse_credentials(json_str: &str) -> Result<VertexCredentials> {
        let json_obj: serde_json::Value = serde_json::from_str(json_str)?;

        match json_obj.get("type").and_then(|t| t.as_str()) {
            Some("service_account") => {
                let key: ServiceAccountKey = serde_json::from_value(json_obj)?;
                Ok(VertexCredentials::ServiceAccount(key))
            }
            Some("external_account") => {
                let config: WorkloadIdentityConfig = serde_json::from_value(json_obj)?;
                Ok(VertexCredentials::WorkloadIdentity(config))
            }
            Some("authorized_user") => {
                let creds: AuthorizedUserCredentials = serde_json::from_value(json_obj)?;
                Ok(VertexCredentials::AuthorizedUser(creds))
            }
            _ => Err(VertexAuthError::UnknownCredentialType),
        }
    }

    /// Get a valid access token
    pub async fn get_access_token(&self) -> Result<String> {
        // Check cache first
        {
            let cache = self.token_cache.read().await;
            if let Some(ref token) = *cache
                && !token.is_expired()
            {
                return Ok(token.token.clone());
            }
        }

        // Fetch new token based on credential type
        let new_token = match &self.credentials {
            VertexCredentials::ServiceAccount(key) => self.get_service_account_token(key).await?,
            VertexCredentials::WorkloadIdentity(config) => {
                self.get_workload_identity_token(config).await?
            }
            VertexCredentials::ApplicationDefault => self.get_adc_token().await?,
            VertexCredentials::AccessToken(token) => AccessToken {
                token: token.clone(),
                expires_at: Utc::now() + Duration::hours(1),
                token_type: "Bearer".to_string(),
            },
            VertexCredentials::AuthorizedUser(creds) => {
                self.get_authorized_user_token(creds).await?
            }
        };

        // Update cache
        let token_string = new_token.token.clone();
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(new_token);
        }

        Ok(token_string)
    }

    /// Get token for service account
    async fn get_service_account_token(&self, key: &ServiceAccountKey) -> Result<AccessToken> {
        use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

        #[derive(Debug, Serialize)]
        struct Claims {
            iss: String,
            scope: String,
            aud: String,
            exp: i64,
            iat: i64,
        }

        let now = Utc::now().timestamp();
        let claims = Claims {
            iss: key.client_email.clone(),
            scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
            aud: key.token_uri.clone(),
            exp: now + 3600,
            iat: now,
        };

        let header = Header::new(Algorithm::RS256);
        let encoding_key = EncodingKey::from_rsa_pem(key.private_key.as_bytes())?;
        let jwt = encode(&header, &claims, &encoding_key)?;

        // Exchange JWT for access token
        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ];

        let response = self
            .http_client
            .post(&key.token_uri)
            .form(&params)
            .send()
            .await?;

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
            token_type: String,
        }

        let token_response: TokenResponse = response.json().await?;

        Ok(AccessToken {
            token: token_response.access_token,
            expires_at: Utc::now() + Duration::seconds(token_response.expires_in),
            token_type: token_response.token_type,
        })
    }

    /// Get token for workload identity
    async fn get_workload_identity_token(
        &self,
        config: &WorkloadIdentityConfig,
    ) -> Result<AccessToken> {
        // Get subject token from credential source
        let subject_token = self.get_subject_token(&config.credential_source).await?;

        // Exchange for access token
        let mut params = HashMap::new();
        params.insert(
            "grant_type",
            "urn:ietf:params:oauth:grant-type:token-exchange",
        );
        params.insert("audience", &config.audience);
        params.insert("subject_token", &subject_token);
        params.insert("subject_token_type", &config.subject_token_type);
        params.insert("scope", "https://www.googleapis.com/auth/cloud-platform");

        let response = self
            .http_client
            .post(&config.token_url)
            .json(&params)
            .send()
            .await?;

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
            token_type: String,
        }

        let token_response: TokenResponse = response.json().await?;

        Ok(AccessToken {
            token: token_response.access_token,
            expires_at: Utc::now() + Duration::seconds(token_response.expires_in),
            token_type: token_response.token_type,
        })
    }

    /// Get subject token from credential source
    async fn get_subject_token(&self, source: &CredentialSource) -> Result<String> {
        if let Some(ref file_path) = source.file {
            // Read token from file
            tokio::fs::read_to_string(file_path)
                .await
                .map_err(VertexAuthError::ReadSubjectTokenFile)
        } else if let Some(ref url) = source.url {
            // Fetch token from URL
            let mut request = self.http_client.get(url);

            if let Some(ref headers) = source.headers {
                for (key, value) in headers {
                    request = request.header(key, value);
                }
            }

            let response = request.send().await?;
            response
                .text()
                .await
                .map_err(VertexAuthError::FetchSubjectToken)
        } else if let Some(ref env_id) = source.environment_id {
            // AWS environment
            if env_id.contains("aws") {
                self.get_aws_token(source).await
            } else {
                Err(VertexAuthError::UnsupportedEnvironmentId(env_id.clone()))
            }
        } else {
            Err(VertexAuthError::MissingCredentialSource)
        }
    }

    /// Get token from AWS metadata service
    async fn get_aws_token(&self, _source: &CredentialSource) -> Result<String> {
        // TODO: Implement AWS metadata service token retrieval
        Err(VertexAuthError::AwsTokenNotImplemented)
    }

    /// Get token using Application Default Credentials
    async fn get_adc_token(&self) -> Result<AccessToken> {
        // Try metadata service (for GCE/Cloud Run/etc)
        let metadata_url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

        let response = self
            .http_client
            .get(metadata_url)
            .header("Metadata-Flavor", "Google")
            .send()
            .await;

        if let Ok(resp) = response {
            #[derive(Deserialize)]
            struct MetadataToken {
                access_token: String,
                expires_in: i64,
                token_type: String,
            }

            if let Ok(token) = resp.json::<MetadataToken>().await {
                return Ok(AccessToken {
                    token: token.access_token,
                    expires_at: Utc::now() + Duration::seconds(token.expires_in),
                    token_type: token.token_type,
                });
            }
        }

        // Fall back to gcloud auth
        Err(VertexAuthError::AdcTokenUnavailable)
    }

    /// Get token for authorized user
    async fn get_authorized_user_token(
        &self,
        creds: &AuthorizedUserCredentials,
    ) -> Result<AccessToken> {
        let grant_type = "refresh_token".to_string();
        let params = [
            ("client_id", &creds.client_id),
            ("client_secret", &creds.client_secret),
            ("refresh_token", &creds.refresh_token),
            ("grant_type", &grant_type),
        ];

        let response = self
            .http_client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await?;

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
            token_type: String,
        }

        let token_response: TokenResponse = response.json().await?;

        Ok(AccessToken {
            token: token_response.access_token,
            expires_at: Utc::now() + Duration::seconds(token_response.expires_in),
            token_type: token_response.token_type,
        })
    }

    /// Get the project ID
    pub async fn get_project_id(&self) -> Result<String> {
        match &self.credentials {
            VertexCredentials::ServiceAccount(key) => Ok(key.project_id.clone()),
            VertexCredentials::WorkloadIdentity(config) => config
                .quota_project_id
                .clone()
                .ok_or(VertexAuthError::MissingProjectId),
            _ => {
                // Try to get from metadata service
                let url = "http://metadata.google.internal/computeMetadata/v1/project/project-id";
                let response = self
                    .http_client
                    .get(url)
                    .header("Metadata-Flavor", "Google")
                    .send()
                    .await?;

                response
                    .text()
                    .await
                    .map_err(VertexAuthError::GetProjectIdFromMetadata)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_token_is_expired() {
        // Token that expires in the future (not expired)
        let token = AccessToken {
            token: "test-token".to_string(),
            expires_at: Utc::now() + Duration::hours(1),
            token_type: "Bearer".to_string(),
        };
        assert!(!token.is_expired());

        // Token that expires in 4 minutes (within 5 min buffer, so expired)
        let token = AccessToken {
            token: "test-token".to_string(),
            expires_at: Utc::now() + Duration::minutes(4),
            token_type: "Bearer".to_string(),
        };
        assert!(token.is_expired());

        // Token that already expired
        let token = AccessToken {
            token: "test-token".to_string(),
            expires_at: Utc::now() - Duration::hours(1),
            token_type: "Bearer".to_string(),
        };
        assert!(token.is_expired());
    }

    #[test]
    fn test_vertex_credentials_variants() {
        let _creds = VertexCredentials::AccessToken("test-token".to_string());
        let _creds = VertexCredentials::ApplicationDefault;
    }

    #[test]
    fn test_vertex_auth_new() {
        let auth = VertexAuth::new(VertexCredentials::AccessToken("test-token".to_string()));
        // Just verify it can be created
        assert!(format!("{:?}", auth).contains("VertexAuth"));
    }

    #[test]
    fn test_service_account_key_structure() {
        let json = r#"{
            "type": "service_account",
            "project_id": "test-project",
            "private_key_id": "key-id",
            "private_key": "-----BEGIN RSA PRIVATE KEY-----\ntest\n-----END RSA PRIVATE KEY-----\n",
            "client_email": "test@test.iam.gserviceaccount.com",
            "client_id": "123456789",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
            "client_x509_cert_url": "https://www.googleapis.com/robot/v1/metadata/x509/test"
        }"#;

        let key: ServiceAccountKey = serde_json::from_str(json).unwrap();
        assert_eq!(key.key_type, "service_account");
        assert_eq!(key.project_id, "test-project");
        assert_eq!(key.client_email, "test@test.iam.gserviceaccount.com");
    }

    #[test]
    fn test_authorized_user_credentials_structure() {
        let json = r#"{
            "client_id": "123.apps.googleusercontent.com",
            "client_secret": "secret",
            "refresh_token": "refresh-token",
            "type": "authorized_user"
        }"#;

        let creds: AuthorizedUserCredentials = serde_json::from_str(json).unwrap();
        assert_eq!(creds.cred_type, "authorized_user");
        assert_eq!(creds.client_id, "123.apps.googleusercontent.com");
    }

    #[test]
    fn test_credential_source_structure() {
        let source = CredentialSource {
            file: Some("/path/to/token".to_string()),
            url: None,
            headers: None,
            environment_id: None,
            regional_cred_verification_url: None,
        };
        assert_eq!(source.file, Some("/path/to/token".to_string()));

        let source_with_url = CredentialSource {
            file: None,
            url: Some("https://metadata.example.com/token".to_string()),
            headers: Some({
                let mut h = HashMap::new();
                h.insert("Authorization".to_string(), "Bearer token".to_string());
                h
            }),
            environment_id: None,
            regional_cred_verification_url: None,
        };
        assert!(source_with_url.headers.is_some());
    }

    #[test]
    fn test_parse_credentials_service_account() {
        let json = r#"{
            "type": "service_account",
            "project_id": "test-project",
            "private_key_id": "key-id",
            "private_key": "-----BEGIN RSA PRIVATE KEY-----\ntest\n-----END RSA PRIVATE KEY-----\n",
            "client_email": "test@test.iam.gserviceaccount.com",
            "client_id": "123456789",
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "auth_provider_x509_cert_url": "https://www.googleapis.com/oauth2/v1/certs",
            "client_x509_cert_url": "https://www.googleapis.com/robot/v1/metadata/x509/test"
        }"#;

        let creds = VertexAuth::parse_credentials(json).unwrap();
        assert!(matches!(creds, VertexCredentials::ServiceAccount(_)));
    }

    #[test]
    fn test_parse_credentials_authorized_user() {
        let json = r#"{
            "client_id": "123.apps.googleusercontent.com",
            "client_secret": "secret",
            "refresh_token": "refresh-token",
            "type": "authorized_user"
        }"#;

        let creds = VertexAuth::parse_credentials(json).unwrap();
        assert!(matches!(creds, VertexCredentials::AuthorizedUser(_)));
    }

    #[test]
    fn test_parse_credentials_unknown_type() {
        let json = r#"{"type": "unknown_type"}"#;
        let result = VertexAuth::parse_credentials(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_workload_identity_config_structure() {
        let config = WorkloadIdentityConfig {
            config_type: "external_account".to_string(),
            audience: "//iam.googleapis.com/projects/123/locations/global/workloadIdentityPools/pool/providers/provider".to_string(),
            subject_token_type: "urn:ietf:params:oauth:token-type:jwt".to_string(),
            service_account_impersonation_url: None,
            token_url: "https://sts.googleapis.com/v1/token".to_string(),
            credential_source: CredentialSource {
                file: Some("/var/run/secrets/token".to_string()),
                url: None,
                headers: None,
                environment_id: None,
                regional_cred_verification_url: None,
            },
            quota_project_id: Some("test-project".to_string()),
        };

        assert_eq!(config.config_type, "external_account");
        assert!(config.quota_project_id.is_some());
    }
}
