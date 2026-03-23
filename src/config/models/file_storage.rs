//! File storage configuration

use serde::{Deserialize, Serialize};

/// File storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStorageConfig {
    /// Storage type (local, s3, etc.)
    pub storage_type: String,
    /// Local storage path
    pub local_path: Option<String>,
    /// S3 configuration
    pub s3: Option<S3Config>,
}

impl Default for FileStorageConfig {
    fn default() -> Self {
        Self {
            storage_type: "local".to_string(),
            local_path: None,
            s3: None,
        }
    }
}

/// S3 configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// S3 bucket name
    pub bucket: String,
    /// AWS region
    pub region: String,
    /// Access key ID
    pub access_key_id: String,
    /// Secret access key
    pub secret_access_key: String,
    /// Endpoint URL (for S3-compatible services)
    pub endpoint: Option<String>,
}

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("access_key_id", &"***REDACTED***")
            .field("secret_access_key", &"***REDACTED***")
            .field("endpoint", &self.endpoint)
            .finish()
    }
}

/// Vector database configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct VectorDbConfig {
    /// Vector DB type (pinecone, weaviate, etc.)
    pub db_type: String,
    /// Connection URL
    pub url: String,
    /// API key
    pub api_key: String,
    /// Index name
    pub index_name: String,
}

impl std::fmt::Debug for VectorDbConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorDbConfig")
            .field("db_type", &self.db_type)
            .field("url", &self.url)
            .field("api_key", &"***REDACTED***")
            .field("index_name", &self.index_name)
            .finish()
    }
}

impl Default for VectorDbConfig {
    fn default() -> Self {
        Self {
            db_type: "pinecone".to_string(),
            url: String::new(),
            api_key: String::new(),
            index_name: "default".to_string(),
        }
    }
}

/// Alerting configuration
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct AlertingConfig {
    /// Enable alerting
    #[serde(default)]
    pub enabled: bool,
    /// Slack webhook URL
    pub slack_webhook: Option<String>,
    /// Email configuration
    pub email: Option<EmailConfig>,
}

impl std::fmt::Debug for AlertingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlertingConfig")
            .field("enabled", &self.enabled)
            .field(
                "slack_webhook",
                &self.slack_webhook.as_ref().map(|_| "***REDACTED***"),
            )
            .field("email", &self.email)
            .finish()
    }
}

/// Email configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    /// SMTP server
    pub smtp_server: String,
    /// SMTP port
    pub smtp_port: u16,
    /// Username
    pub username: String,
    /// Password
    pub password: String,
    /// From address
    pub from_address: String,
    /// To addresses
    pub to_addresses: Vec<String>,
}

impl std::fmt::Debug for EmailConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailConfig")
            .field("smtp_server", &self.smtp_server)
            .field("smtp_port", &self.smtp_port)
            .field("username", &self.username)
            .field("password", &"***REDACTED***")
            .field("from_address", &self.from_address)
            .field("to_addresses", &self.to_addresses)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== FileStorageConfig Tests ====================

    #[test]
    fn test_file_storage_config_default() {
        let config = FileStorageConfig::default();
        assert_eq!(config.storage_type, "local");
        // local_path is None by default; resolved to absolute path at runtime
        assert!(config.local_path.is_none());
        assert!(config.s3.is_none());
    }

    #[test]
    fn test_file_storage_config_local() {
        let config = FileStorageConfig {
            storage_type: "local".to_string(),
            local_path: Some("/var/data".to_string()),
            s3: None,
        };
        assert_eq!(config.storage_type, "local");
        assert_eq!(config.local_path, Some("/var/data".to_string()));
    }

    #[test]
    fn test_file_storage_config_s3() {
        let s3 = S3Config {
            bucket: "my-bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "secret".to_string(),
            endpoint: None,
        };

        let config = FileStorageConfig {
            storage_type: "s3".to_string(),
            local_path: None,
            s3: Some(s3),
        };
        assert_eq!(config.storage_type, "s3");
        assert!(config.s3.is_some());
    }

    #[test]
    fn test_file_storage_config_serialization() {
        let config = FileStorageConfig::default();
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["storage_type"], "local");
        assert!(json["local_path"].is_null());
    }

    #[test]
    fn test_file_storage_config_clone() {
        let config = FileStorageConfig::default();
        let cloned = config.clone();
        assert_eq!(config.storage_type, cloned.storage_type);
    }

    // ==================== S3Config Tests ====================

    #[test]
    fn test_s3_config_structure() {
        let config = S3Config {
            bucket: "test-bucket".to_string(),
            region: "eu-west-1".to_string(),
            access_key_id: "access".to_string(),
            secret_access_key: "secret".to_string(),
            endpoint: Some("https://s3.custom.com".to_string()),
        };
        assert_eq!(config.bucket, "test-bucket");
        assert_eq!(config.region, "eu-west-1");
        assert!(config.endpoint.is_some());
    }

    #[test]
    fn test_s3_config_serialization() {
        let config = S3Config {
            bucket: "bucket".to_string(),
            region: "us-west-2".to_string(),
            access_key_id: "key".to_string(),
            secret_access_key: "secret".to_string(),
            endpoint: None,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["bucket"], "bucket");
        assert_eq!(json["region"], "us-west-2");
    }

    #[test]
    fn test_s3_config_deserialization() {
        let json = r#"{
            "bucket": "my-bucket",
            "region": "ap-southeast-1",
            "access_key_id": "AKIA...",
            "secret_access_key": "secret123"
        }"#;
        let config: S3Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.bucket, "my-bucket");
        assert!(config.endpoint.is_none());
    }

    #[test]
    fn test_s3_config_clone() {
        let config = S3Config {
            bucket: "clone-test".to_string(),
            region: "us-east-1".to_string(),
            access_key_id: "key".to_string(),
            secret_access_key: "secret".to_string(),
            endpoint: None,
        };
        let cloned = config.clone();
        assert_eq!(config.bucket, cloned.bucket);
    }

    // ==================== VectorDbConfig Tests ====================

    #[test]
    fn test_vector_db_config_default() {
        let config = VectorDbConfig::default();
        assert_eq!(config.db_type, "pinecone");
        assert!(config.url.is_empty());
        assert!(config.api_key.is_empty());
        assert_eq!(config.index_name, "default");
    }

    #[test]
    fn test_vector_db_config_structure() {
        let config = VectorDbConfig {
            db_type: "weaviate".to_string(),
            url: "http://weaviate:8080".to_string(),
            api_key: "weaviate-key".to_string(),
            index_name: "embeddings".to_string(),
        };
        assert_eq!(config.db_type, "weaviate");
        assert_eq!(config.index_name, "embeddings");
    }

    #[test]
    fn test_vector_db_config_serialization() {
        let config = VectorDbConfig {
            db_type: "qdrant".to_string(),
            url: "http://qdrant:6333".to_string(),
            api_key: "qdrant-key".to_string(),
            index_name: "vectors".to_string(),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["db_type"], "qdrant");
        assert_eq!(json["url"], "http://qdrant:6333");
    }

    #[test]
    fn test_vector_db_config_clone() {
        let config = VectorDbConfig::default();
        let cloned = config.clone();
        assert_eq!(config.db_type, cloned.db_type);
    }

    // ==================== AlertingConfig Tests ====================

    #[test]
    fn test_alerting_config_default() {
        let config = AlertingConfig::default();
        assert!(!config.enabled);
        assert!(config.slack_webhook.is_none());
        assert!(config.email.is_none());
    }

    #[test]
    fn test_alerting_config_with_slack() {
        let config = AlertingConfig {
            enabled: true,
            slack_webhook: Some("https://hooks.slack.com/xxx".to_string()),
            email: None,
        };
        assert!(config.enabled);
        assert!(config.slack_webhook.is_some());
    }

    #[test]
    fn test_alerting_config_serialization() {
        let config = AlertingConfig {
            enabled: true,
            slack_webhook: Some("https://slack.webhook".to_string()),
            email: None,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn test_alerting_config_clone() {
        let config = AlertingConfig::default();
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
    }

    // ==================== EmailConfig Tests ====================

    #[test]
    fn test_email_config_structure() {
        let config = EmailConfig {
            smtp_server: "smtp.example.com".to_string(),
            smtp_port: 587,
            username: "user@example.com".to_string(),
            password: "password".to_string(),
            from_address: "alerts@example.com".to_string(),
            to_addresses: vec!["admin@example.com".to_string()],
        };
        assert_eq!(config.smtp_server, "smtp.example.com");
        assert_eq!(config.smtp_port, 587);
        assert_eq!(config.to_addresses.len(), 1);
    }

    #[test]
    fn test_email_config_serialization() {
        let config = EmailConfig {
            smtp_server: "mail.test.com".to_string(),
            smtp_port: 465,
            username: "sender".to_string(),
            password: "pass".to_string(),
            from_address: "from@test.com".to_string(),
            to_addresses: vec!["to1@test.com".to_string(), "to2@test.com".to_string()],
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["smtp_port"], 465);
        assert!(json["to_addresses"].is_array());
    }

    #[test]
    fn test_email_config_deserialization() {
        let json = r#"{
            "smtp_server": "smtp.gmail.com",
            "smtp_port": 587,
            "username": "user",
            "password": "pass",
            "from_address": "from@gmail.com",
            "to_addresses": ["to@gmail.com"]
        }"#;
        let config: EmailConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.smtp_server, "smtp.gmail.com");
        assert_eq!(config.smtp_port, 587);
    }

    #[test]
    fn test_email_config_clone() {
        let config = EmailConfig {
            smtp_server: "smtp.clone.com".to_string(),
            smtp_port: 25,
            username: "clone".to_string(),
            password: "pass".to_string(),
            from_address: "clone@test.com".to_string(),
            to_addresses: vec![],
        };
        let cloned = config.clone();
        assert_eq!(config.smtp_server, cloned.smtp_server);
    }
}
