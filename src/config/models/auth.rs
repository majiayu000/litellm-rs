//! Authentication configuration

use super::*;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Authentication configuration
#[derive(Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable JWT authentication
    #[serde(default = "default_auth_enable_jwt")]
    pub enable_jwt: bool,
    /// Enable API key authentication
    #[serde(default = "default_true")]
    pub enable_api_key: bool,
    /// JWT secret
    #[serde(default)]
    pub jwt_secret: String,
    /// JWT expiration in seconds
    #[serde(default = "default_jwt_expiration")]
    pub jwt_expiration: u64,
    /// API key header name
    #[serde(default = "default_api_key_header")]
    pub api_key_header: String,
    /// HMAC secret for API key hashing. When set, API keys are hashed with
    /// HMAC-SHA256(secret, key) instead of plain SHA-256. Strongly recommended
    /// for production deployments.
    #[serde(default)]
    pub api_key_hmac_secret: Option<String>,
    /// RBAC configuration
    #[serde(default)]
    pub rbac: RbacConfig,
}

impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConfig")
            .field("enable_jwt", &self.enable_jwt)
            .field("enable_api_key", &self.enable_api_key)
            .field("jwt_secret", &"[REDACTED]")
            .field("jwt_expiration", &self.jwt_expiration)
            .field("api_key_header", &self.api_key_header)
            .field(
                "api_key_hmac_secret",
                &self.api_key_hmac_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("rbac", &self.rbac)
            .finish()
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enable_jwt: default_auth_enable_jwt(),
            enable_api_key: true,
            jwt_secret: String::new(),
            jwt_expiration: default_jwt_expiration(),
            api_key_header: default_api_key_header(),
            api_key_hmac_secret: None,
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
        if other.api_key_hmac_secret.is_some() {
            self.api_key_hmac_secret = other.api_key_hmac_secret;
        }
        self.rbac = self.rbac.merge(other.rbac);
        self
    }

    /// Validate authentication configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate JWT secret strength
        if self.enable_jwt {
            if self.jwt_secret.is_empty() {
                return Err(
                    "JWT authentication is enabled but jwt_secret is empty. \
                     Set a secure jwt_secret (>= 32 bytes for HS256 256-bit minimum) or disable JWT auth with enable_jwt: false"
                        .to_string(),
                );
            }

            if self.jwt_secret.len() < 32 {
                return Err(
                    "JWT secret must be at least 32 bytes (256-bit) for HS256 security".to_string(),
                );
            }

            if is_forbidden_jwt_placeholder(&self.jwt_secret) {
                return Err("JWT secret must not use default values. Please generate a secure random secret.".to_string());
            }

            let has_lowercase = self.jwt_secret.chars().any(|c| c.is_ascii_lowercase());
            let has_uppercase = self.jwt_secret.chars().any(|c| c.is_ascii_uppercase());

            // Check for common weak patterns
            if has_lowercase && !has_uppercase {
                return Err(
                    "JWT secret should include uppercase and lowercase letters; generate a high-entropy mixed secret"
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

fn default_auth_enable_jwt() -> bool {
    false
}

fn is_forbidden_jwt_placeholder(secret: &str) -> bool {
    let normalized = secret.trim().to_ascii_lowercase();
    let compact = normalized.replace(['_', '-', ' '], "");

    normalized == "your-secret-key"
        || normalized == "change-me"
        || compact.contains("yoursecretkey")
        || compact.contains("changeme")
        || compact.contains("replacewith")
        || compact.contains("placeholder")
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

    if config.enable_api_key && config.api_key_hmac_secret.is_none() {
        warn!(
            "API key authentication is enabled without api_key_hmac_secret. \
             API keys are hashed with plain SHA-256, which is vulnerable to \
             offline brute-force if the database is compromised. Set \
             api_key_hmac_secret to a secure random value (>= 32 chars) to \
             use HMAC-SHA256 instead."
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
        assert!(!config.enable_jwt);
        assert!(config.enable_api_key);
        assert!(config.jwt_secret.is_empty());
        assert_eq!(config.jwt_expiration, 86400); // 24 hours
        assert_eq!(config.api_key_header, "Authorization");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_auth_config_deserialize_missing_enable_jwt_defaults_disabled() {
        let json = r#"{
            "enable_api_key": true,
            "jwt_expiration": 3600,
            "api_key_header": "Authorization"
        }"#;
        let config: AuthConfig = serde_json::from_str(json).unwrap();

        assert!(!config.enable_jwt);
        assert!(config.enable_api_key);
        assert!(config.jwt_secret.is_empty());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_auth_config_structure() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: false,
            jwt_secret: "A".repeat(64),
            jwt_expiration: 7200,
            api_key_header: "Authorization".to_string(),
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_secret_checks_bytes_not_chars() {
        // 16 CJK characters = 48 UTF-8 bytes (>= 32), but only 16 chars
        // Mixed with ASCII to pass the weak-pattern check
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "A1!abcde\u{4e00}\u{4e01}\u{4e02}\u{4e03}\u{4e04}\u{4e05}\u{4e06}\u{4e07}\u{4e08}\u{4e09}\u{4e0a}\u{4e0b}".to_string(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            api_key_hmac_secret: None,
            rbac: RbacConfig::default(),
        };
        // 8 ASCII bytes + 12 CJK * 3 bytes = 44 bytes >= 32, should pass length check
        assert!(config.jwt_secret.len() >= 32);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_auth_config_validate_default_secret() {
        for secret in [
            "your-secret-key",
            "change-me",
            "ReplaceWithAtLeast32CharsMixedCaseAndSymbols123!",
            "${LITELLM_JWT_SECRET}",
        ] {
            let config = AuthConfig {
                enable_jwt: true,
                enable_api_key: true,
                jwt_secret: secret.to_string(),
                jwt_expiration: 3600,
                api_key_header: "X-API-Key".to_string(),
                api_key_hmac_secret: None,
                rbac: RbacConfig::default(),
            };
            assert!(
                config.validate().is_err(),
                "placeholder secret should be rejected: {secret}"
            );
        }
    }

    #[test]
    fn test_auth_config_validate_weak_secret() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "a".repeat(64), // all lowercase
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            api_key_hmac_secret: None,
            rbac: RbacConfig::default(),
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_auth_config_validate_lowercase_with_digits_secret() {
        let config = AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "lowercase-secret-with-digits-1234567890".to_string(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
            api_key_hmac_secret: None,
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
