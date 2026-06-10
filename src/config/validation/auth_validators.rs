//! Authentication configuration validators
//!
//! This module provides validation implementations for authentication-related
//! configuration structures including AuthConfig and RbacConfig.

use super::trait_def::Validate;
use crate::config::models::auth::{AuthConfig, RbacConfig};
use tracing::debug;

impl Validate for AuthConfig {
    fn validate(&self) -> Result<(), String> {
        debug!("Validating auth configuration");
        AuthConfig::validate(self)?;
        self.rbac.validate()?;

        Ok(())
    }
}

impl Validate for RbacConfig {
    fn validate(&self) -> Result<(), String> {
        if self.enabled && self.default_role.is_empty() {
            return Err("Default role cannot be empty when RBAC is enabled".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::trait_def::Validate;
    use super::*; // Import the trait explicitly

    fn create_valid_auth_config() -> AuthConfig {
        AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaaA1!".to_string(),
            jwt_expiration: 3600, // 1 hour
            api_key_header: "X-API-Key".to_string(),
            api_key_hmac_secret: None,
            allow_anonymous: false,
            rbac: RbacConfig::default(),
        }
    }

    // Helper to call the Validate trait method explicitly
    fn validate_config<T: Validate>(config: &T) -> Result<(), String> {
        Validate::validate(config)
    }

    // ==================== AuthConfig Validation Tests ====================

    #[test]
    fn test_auth_config_valid() {
        let config = create_valid_auth_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_auth_config_empty_jwt_secret() {
        let mut config = create_valid_auth_config();
        config.jwt_secret = "".to_string();

        let result = validate_config(&config);
        assert!(
            result.is_err(),
            "Expected validation to fail for empty JWT secret"
        );
        let err = result.unwrap_err();
        assert!(err.contains("jwt_secret is empty"), "Got error: {}", err);
    }

    #[test]
    fn test_auth_config_empty_jwt_secret_when_jwt_disabled() {
        let mut config = create_valid_auth_config();
        config.enable_jwt = false;
        config.jwt_secret = "".to_string();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_auth_config_short_jwt_secret() {
        let mut config = create_valid_auth_config();
        config.jwt_secret = "short".to_string(); // Less than 32 bytes

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 32 bytes"));
    }

    #[test]
    fn test_auth_config_jwt_secret_exactly_32_chars() {
        let mut config = create_valid_auth_config();
        config.jwt_secret = "AaAaAaAaAaAaAaAaAaAaAaAaAaAaAa1!".to_string();

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_auth_config_jwt_secret_long() {
        let mut config = create_valid_auth_config();
        config.jwt_secret = "Abc123!Z".repeat(13);

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_auth_config_zero_jwt_expiration() {
        let mut config = create_valid_auth_config();
        config.jwt_expiration = 0;

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 5 minutes"));
    }

    #[test]
    fn test_auth_config_max_jwt_expiration() {
        let mut config = create_valid_auth_config();
        config.jwt_expiration = 86400 * 30; // Exactly 30 days

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_auth_config_jwt_expiration_too_long() {
        let mut config = create_valid_auth_config();
        config.jwt_expiration = 86400 * 30 + 1; // More than 30 days

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("30 days"));
    }

    #[test]
    fn test_auth_config_empty_api_key_header() {
        let mut config = create_valid_auth_config();
        config.api_key_header = "".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("API key header"));
    }

    #[test]
    fn test_auth_config_custom_api_key_header() {
        let mut config = create_valid_auth_config();
        config.api_key_header = "Authorization".to_string();

        assert!(validate_config(&config).is_ok());
    }

    // ==================== RbacConfig Validation Tests ====================

    #[test]
    fn test_rbac_config_disabled_empty_role() {
        let config = RbacConfig {
            enabled: false,
            default_role: "".to_string(),
            ..Default::default()
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_rbac_config_enabled_with_role() {
        let config = RbacConfig {
            enabled: true,
            default_role: "user".to_string(),
            ..Default::default()
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_rbac_config_enabled_empty_role() {
        let config = RbacConfig {
            enabled: true,
            default_role: "".to_string(),
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Default role cannot be empty"));
    }

    // ==================== AuthConfig with RbacConfig Tests ====================

    #[test]
    fn test_auth_config_with_valid_rbac() {
        let mut config = create_valid_auth_config();
        config.rbac = RbacConfig {
            enabled: true,
            default_role: "admin".to_string(),
            ..Default::default()
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_auth_config_with_invalid_rbac() {
        let mut config = create_valid_auth_config();
        config.rbac = RbacConfig {
            enabled: true,
            default_role: "".to_string(),
            ..Default::default()
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Default role"));
    }
}
