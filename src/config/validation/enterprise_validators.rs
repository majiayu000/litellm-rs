//! Enterprise configuration validators
//!
//! This module provides validation implementations for enterprise-related
//! configuration structures including EnterpriseConfig and SsoConfig.

use super::trait_def::Validate;
use crate::config::models::enterprise::{EnterpriseConfig, SsoConfig};

impl Validate for EnterpriseConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(sso) = &self.sso {
            sso.validate()?;
        }

        Ok(())
    }
}

impl Validate for SsoConfig {
    fn validate(&self) -> Result<(), String> {
        let supported_providers = ["saml", "oidc", "oauth2"];
        if !supported_providers.contains(&self.provider.as_str()) {
            return Err(format!(
                "Unsupported SSO provider: {}. Supported providers: {:?}",
                self.provider, supported_providers
            ));
        }

        if self.client_id.is_empty() {
            return Err("SSO client ID cannot be empty".to_string());
        }

        if self.client_secret.is_empty() {
            return Err("SSO client secret cannot be empty".to_string());
        }

        if self.redirect_url.is_empty() {
            return Err("SSO redirect URL cannot be empty".to_string());
        }

        Ok(())
    }
}
