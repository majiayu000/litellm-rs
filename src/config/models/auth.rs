//! Authentication configuration

use super::*;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable JWT authentication
    #[serde(default = "default_true")]
    pub enable_jwt: bool,
    /// Enable API key authentication
    #[serde(default = "default_true")]
    pub enable_api_key: bool,
    /// JWT secret
    pub jwt_secret: String,
    /// JWT expiration in seconds
    #[serde(default = "default_jwt_expiration")]
    pub jwt_expiration: u64,
    /// API key header name
    #[serde(default = "default_api_key_header")]
    pub api_key_header: String,
    /// RBAC configuration
    #[serde(default)]
    pub rbac: RbacConfig,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: String::new(),
            jwt_expiration: default_jwt_expiration(),
            api_key_header: default_api_key_header(),
            rbac: RbacConfig::default(),
        }
    }
}

impl AuthConfig {
    /// Merge auth configurations
    pub fn merge(mut self, other: Self) -> Self {
        if other.enable_jwt {
            self.enable_jwt = true;
        }
        if other.enable_api_key {
            self.enable_api_key = true;
        }
        if !other.jwt_secret.is_empty() {
            self.jwt_secret = other.jwt_secret;
        }
        if other.jwt_expiration != default_jwt_expiration() {
            self.jwt_expiration = other.jwt_expiration;
        }
        if other.api_key_header != default_api_key_header() {
            self.api_key_header = other.api_key_header;
        }
        self.rbac = self.rbac.merge(other.rbac);
        self
    }

    /// Validate authentication configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate JWT secret strength
        if self.enable_jwt {
            if self.jwt_secret.len() < 32 {
                return Err(
                    "JWT secret must be at least 32 characters long for security".to_string(),
                );
            }

            if self.jwt_secret == "your-secret-key" || self.jwt_secret == "change-me" {
                return Err("JWT secret must not use default values. Please generate a secure random secret.".to_string());
            }

            // Check for common weak patterns
            if self.jwt_secret.chars().all(|c| c.is_ascii_lowercase()) {
                return Err(
                    "JWT secret should contain mixed case letters, numbers, and special characters"
                        .to_string(),
                );
            }
        }

        // Validate JWT expiration
        if self.jwt_expiration < 300 {
            return Err("JWT expiration should be at least 5 minutes (300 seconds)".to_string());
        }

        if self.jwt_expiration > 86400 * 30 {
            return Err(
                "JWT expiration should not exceed 30 days for security reasons".to_string(),
            );
        }

        // Validate API key header
        if self.enable_api_key && self.api_key_header.is_empty() {
            return Err(
                "API key header name cannot be empty when API key auth is enabled".to_string(),
            );
        }

        Ok(())
    }

    /// Check if authentication is properly configured for production
    pub fn is_production_ready(&self) -> bool {
        self.enable_jwt || self.enable_api_key
    }
}

/// RBAC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacConfig {
    /// Enable RBAC
    #[serde(default)]
    pub enabled: bool,
    /// Default role for new users
    #[serde(default = "default_role")]
    pub default_role: String,
    /// Admin roles
    #[serde(default = "default_admin_roles")]
    pub admin_roles: Vec<String>,
}

impl Default for RbacConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_role: default_role(),
            admin_roles: default_admin_roles(),
        }
    }
}

impl RbacConfig {
    /// Merge RBAC configurations
    pub fn merge(mut self, other: Self) -> Self {
        if other.enabled {
            self.enabled = other.enabled;
        }
        if other.default_role != default_role() {
            self.default_role = other.default_role;
        }
        if other.admin_roles != default_admin_roles() {
            self.admin_roles = other.admin_roles;
        }
        self
    }
}

/// Warn about insecure configuration in development
pub fn warn_insecure_config(config: &AuthConfig) {
    if !config.is_production_ready() {
        warn!(
            "Authentication is disabled! This is insecure for production use. Enable JWT or API key authentication before deploying to production."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secure_jwt_secret() -> String {
        "CustomSecret123!@#456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string()
    }

    // ==================== RbacConfig Tests ====================

    #[test]
    fn test_rbac_config_default() {
        let config = RbacConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.default_role, "user");
        assert!(!config.admin_roles.is_empty());
    }

    #[test]
    fn test_rbac_config_structure() {
        let config = RbacConfig {
            enabled: true,
            default_role: "viewer".to_string(),
            admin_roles: vec!["admin".to_string(), "superadmin".to_string()],
        };
        assert!(config.enabled);
        assert_eq!(config.default_role, "viewer");
        assert_eq!(config.admin_roles.len(), 2);
    }

    #[test]
    fn test_rbac_config_serialization() {
        let config = RbacConfig {
            enabled: true,
            default_role: "editor".to_string(),
            admin_roles: vec!["admin".to_string()],
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["default_role"], "editor");
    }

    #[test]
    fn test_rbac_config_deserialization() {
        let json = r#"{"enabled": true, "default_role": "guest", "admin_roles": ["admin"]}"#;
        let config: RbacConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.default_role, "guest");
    }

    #[test]
    fn test_rbac_config_merge_enabled() {
        let base = RbacConfig::default();
        let other = RbacConfig {
            enabled: true,
            default_role: "user".to_string(),
            admin_roles: default_admin_roles(),
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
    }

    #[test]
    fn test_rbac_config_merge_role() {
        let base = RbacConfig::default();
        let other = RbacConfig {
            enabled: false,
            default_role: "custom_role".to_string(),
            admin_roles: default_admin_roles(),
        };
        let merged = base.merge(other);
        assert_eq!(merged.default_role, "custom_role");
    }

    #[test]
    fn test_rbac_config_clone() {
        let config = RbacConfig {
            enabled: true,
            default_role: "clone_test".to_string(),
            admin_roles: vec!["admin".to_string()],
        };
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
        assert_eq!(config.default_role, cloned.default_role);
    }

    // ==================== AuthConfig Tests ====================

    #[test]
    fn test_auth_config_default() {
        let config = AuthConfig::default();
        assert!(config.enable_jwt);
        assert!(config.enable_api_key);
        assert!(config.jwt_secret.is_empty());
        assert_eq!(config.jwt_expiration, 86400); // 24 hours
        assert_eq!(config.api_key_header, "Authorization");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_structure() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: false,
            jwt_secret: "A".repeat(64),
            jwt_expiration: 7200,
            api_key_header: "Authorization".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.enable_jwt);
        assert!(!config.enable_api_key);
        assert_eq!(config.jwt_expiration, 7200);
    }

    #[test]
    fn test_auth_config_serialization() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "X".repeat(64),
            jwt_expiration: 1800,
            api_key_header: "X-Token".to_string(),
            rbac: RbacConfig::default(),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enable_jwt"], true);
        assert_eq!(json["jwt_expiration"], 1800);
    }

    #[test]
    fn test_auth_config_validate_short_secret() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "short".to_string(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_default_secret() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "your-secret-key".to_string(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_weak_secret() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "a".repeat(64), // all lowercase
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_short_expiration() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: secure_jwt_secret(),
            jwt_expiration: 100, // less than 300
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_long_expiration() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: secure_jwt_secret(),
            jwt_expiration: 86400 * 31, // more than 30 days
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_empty_header() {
        let config = AuthConfig {
            enable_jwt: false,
            enable_api_key: true,
            jwt_secret: String::new(),
            jwt_expiration: 3600,
            api_key_header: "".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_success() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: secure_jwt_secret(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_auth_config_is_production_ready() {
        let config = AuthConfig::default();
        assert!(config.is_production_ready());

        let disabled = AuthConfig {
            enable_jwt: false,
            enable_api_key: false,
            jwt_secret: String::new(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        assert!(!disabled.is_production_ready());
    }

    #[test]
    fn test_auth_config_merge_jwt_secret() {
        let base = AuthConfig::default();
        let other = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "CustomSecret123!@#456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        let merged = base.merge(other);
        assert!(merged.jwt_secret.contains("CustomSecret123"));
    }

    #[test]
    fn test_auth_config_merge_expiration() {
        let base = AuthConfig::default();
        let other = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: String::new(),
            jwt_expiration: 7200,
            api_key_header: "X-API-Key".to_string(),
            rbac: RbacConfig::default(),
        };
        let merged = base.merge(other);
        assert_eq!(merged.jwt_expiration, 7200);
    }

    #[test]
    fn test_auth_config_clone() {
        let config = AuthConfig::default();
        let cloned = config.clone();
        assert_eq!(config.enable_jwt, cloned.enable_jwt);
        assert_eq!(config.jwt_expiration, cloned.jwt_expiration);
    }
}
