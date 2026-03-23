//! Enterprise configuration

use serde::{Deserialize, Serialize};

/// Enterprise configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnterpriseConfig {
    /// Enable enterprise features
    #[serde(default)]
    pub enabled: bool,
    /// SSO configuration
    pub sso: Option<SsoConfig>,
    /// Enable audit logging
    #[serde(default)]
    pub audit_logging: bool,
    /// Enable advanced analytics
    #[serde(default)]
    pub advanced_analytics: bool,
}

impl EnterpriseConfig {
    /// Merge enterprise configurations
    pub fn merge(mut self, other: Self) -> Self {
        if other.enabled {
            self.enabled = true;
        }
        if other.sso.is_some() {
            self.sso = other.sso;
        }
        if other.audit_logging {
            self.audit_logging = true;
        }
        if other.advanced_analytics {
            self.advanced_analytics = true;
        }
        self
    }
}

/// SSO configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct SsoConfig {
    /// SSO provider
    pub provider: String,
    /// Client ID
    pub client_id: String,
    /// Client secret
    pub client_secret: String,
    /// Redirect URL
    pub redirect_url: String,
    /// Additional settings
    #[serde(default)]
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}

impl std::fmt::Debug for SsoConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsoConfig")
            .field("provider", &self.provider)
            .field("client_id", &self.client_id)
            .field("client_secret", &"***REDACTED***")
            .field("redirect_url", &self.redirect_url)
            .field("settings", &self.settings)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== SsoConfig Tests ====================

    #[test]
    fn test_sso_config_structure() {
        let config = SsoConfig {
            provider: "okta".to_string(),
            client_id: "client-123".to_string(),
            client_secret: "secret-456".to_string(),
            redirect_url: "https://app.example.com/callback".to_string(),
            settings: std::collections::HashMap::new(),
        };
        assert_eq!(config.provider, "okta");
        assert_eq!(config.client_id, "client-123");
    }

    #[test]
    fn test_sso_config_with_settings() {
        let mut settings = std::collections::HashMap::new();
        settings.insert("domain".to_string(), serde_json::json!("example.okta.com"));
        settings.insert(
            "scopes".to_string(),
            serde_json::json!(["openid", "profile"]),
        );

        let config = SsoConfig {
            provider: "auth0".to_string(),
            client_id: "auth0-client".to_string(),
            client_secret: "auth0-secret".to_string(),
            redirect_url: "https://app.example.com/auth/callback".to_string(),
            settings,
        };
        assert_eq!(config.settings.len(), 2);
    }

    #[test]
    fn test_sso_config_serialization() {
        let config = SsoConfig {
            provider: "google".to_string(),
            client_id: "google-client".to_string(),
            client_secret: "google-secret".to_string(),
            redirect_url: "https://app.example.com/oauth".to_string(),
            settings: std::collections::HashMap::new(),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["provider"], "google");
        assert_eq!(json["client_id"], "google-client");
        assert_eq!(json["redirect_url"], "https://app.example.com/oauth");
    }

    #[test]
    fn test_sso_config_deserialization() {
        let json = r#"{
            "provider": "azure",
            "client_id": "azure-client",
            "client_secret": "azure-secret",
            "redirect_url": "https://app.example.com/azure/callback"
        }"#;
        let config: SsoConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.provider, "azure");
        assert!(config.settings.is_empty());
    }

    #[test]
    fn test_sso_config_clone() {
        let config = SsoConfig {
            provider: "okta".to_string(),
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            redirect_url: "https://example.com".to_string(),
            settings: std::collections::HashMap::new(),
        };
        let cloned = config.clone();
        assert_eq!(config.provider, cloned.provider);
        assert_eq!(config.client_id, cloned.client_id);
    }

    // ==================== EnterpriseConfig Tests ====================

    #[test]
    fn test_enterprise_config_default() {
        let config = EnterpriseConfig::default();
        assert!(!config.enabled);
        assert!(config.sso.is_none());
        assert!(!config.audit_logging);
        assert!(!config.advanced_analytics);
    }

    #[test]
    fn test_enterprise_config_structure() {
        let config = EnterpriseConfig {
            enabled: true,
            sso: None,
            audit_logging: true,
            advanced_analytics: true,
        };
        assert!(config.enabled);
        assert!(config.audit_logging);
        assert!(config.advanced_analytics);
    }

    #[test]
    fn test_enterprise_config_with_sso() {
        let sso = SsoConfig {
            provider: "okta".to_string(),
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            redirect_url: "https://example.com/callback".to_string(),
            settings: std::collections::HashMap::new(),
        };

        let config = EnterpriseConfig {
            enabled: true,
            sso: Some(sso),
            audit_logging: true,
            advanced_analytics: false,
        };
        assert!(config.sso.is_some());
        assert_eq!(config.sso.as_ref().unwrap().provider, "okta");
    }

    #[test]
    fn test_enterprise_config_serialization() {
        let config = EnterpriseConfig {
            enabled: true,
            sso: None,
            audit_logging: true,
            advanced_analytics: true,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["audit_logging"], true);
        assert_eq!(json["advanced_analytics"], true);
    }

    #[test]
    fn test_enterprise_config_deserialization() {
        let json = r#"{
            "enabled": true,
            "audit_logging": true,
            "advanced_analytics": false
        }"#;
        let config: EnterpriseConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert!(config.audit_logging);
        assert!(!config.advanced_analytics);
    }

    #[test]
    fn test_enterprise_config_merge_enabled() {
        let base = EnterpriseConfig::default();
        let other = EnterpriseConfig {
            enabled: true,
            sso: None,
            audit_logging: false,
            advanced_analytics: false,
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
    }

    #[test]
    fn test_enterprise_config_merge_sso() {
        let base = EnterpriseConfig::default();
        let sso = SsoConfig {
            provider: "okta".to_string(),
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            redirect_url: "https://example.com".to_string(),
            settings: std::collections::HashMap::new(),
        };
        let other = EnterpriseConfig {
            enabled: false,
            sso: Some(sso),
            audit_logging: false,
            advanced_analytics: false,
        };
        let merged = base.merge(other);
        assert!(merged.sso.is_some());
    }

    #[test]
    fn test_enterprise_config_merge_audit() {
        let base = EnterpriseConfig::default();
        let other = EnterpriseConfig {
            enabled: false,
            sso: None,
            audit_logging: true,
            advanced_analytics: false,
        };
        let merged = base.merge(other);
        assert!(merged.audit_logging);
    }

    #[test]
    fn test_enterprise_config_clone() {
        let config = EnterpriseConfig {
            enabled: true,
            sso: None,
            audit_logging: true,
            advanced_analytics: true,
        };
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
        assert_eq!(config.audit_logging, cloned.audit_logging);
    }
}
