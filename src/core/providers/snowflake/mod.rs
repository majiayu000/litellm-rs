//! Snowflake Cortex OpenAI-compatible contract.

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::ProviderError;
use crate::core::providers::enterprise::{EnterpriseOpenAiProvider, EnterpriseOpenAiSettings};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type SnowflakeProvider = EnterpriseOpenAiProvider;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnowflakeTokenType {
    KeypairJwt,
    Oauth,
    ProgrammaticAccessToken,
}

impl SnowflakeTokenType {
    fn header_value(self) -> &'static str {
        match self {
            Self::KeypairJwt => "KEYPAIR_JWT",
            Self::Oauth => "OAUTH",
            Self::ProgrammaticAccessToken => "PROGRAMMATIC_ACCESS_TOKEN",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnowflakeConfig {
    pub account_identifier: String,
    pub api_key: String,
    pub token_type: SnowflakeTokenType,
    #[serde(default)]
    pub endpoint_access: ProviderEndpointAccess,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub models: Vec<String>,
}

impl std::fmt::Debug for SnowflakeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnowflakeConfig")
            .field("account_identifier", &self.account_identifier)
            .field("api_key", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .field("endpoint_access", &self.endpoint_access)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("models", &self.models)
            .finish()
    }
}

impl SnowflakeConfig {
    pub fn api_base(&self) -> Result<String, ProviderError> {
        let account = self.account_identifier.trim();
        if account.is_empty()
            || !account
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            return Err(ProviderError::configuration(
                "snowflake",
                "account_identifier contains invalid characters",
            ));
        }
        Ok(format!(
            "https://{account}.snowflakecomputing.com/api/v2/cortex/v1"
        ))
    }

    pub async fn build(self) -> Result<SnowflakeProvider, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::configuration(
                "snowflake",
                "api_key is required",
            ));
        }
        let mut headers = HashMap::new();
        headers.insert(
            "X-Snowflake-Authorization-Token-Type".to_string(),
            self.token_type.header_value().to_string(),
        );
        EnterpriseOpenAiProvider::new(
            "snowflake",
            EnterpriseOpenAiSettings {
                api_base: self.api_base()?,
                api_key: self.api_key,
                model_prefix: "snowflake/",
                endpoint_access: self.endpoint_access,
                timeout: self.timeout,
                max_retries: self.max_retries,
                headers,
                models: self.models,
            },
        )
        .await
    }
}

const fn default_timeout() -> u64 {
    60
}
const fn default_retries() -> u32 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_identity_builds_exact_cortex_base() {
        let config = SnowflakeConfig {
            account_identifier: "org-account".to_string(),
            api_key: "jwt".to_string(),
            token_type: SnowflakeTokenType::KeypairJwt,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
            models: Vec::new(),
        };
        assert_eq!(
            config.api_base().expect("valid account"),
            "https://org-account.snowflakecomputing.com/api/v2/cortex/v1"
        );
        assert_eq!(config.token_type.header_value(), "KEYPAIR_JWT");
    }
    #[test]
    fn account_identity_rejects_host_injection() {
        for account_identifier in [
            "acct/@evil.test",
            "user:password@acct",
            "acct?tenant=other",
            "acct#fragment",
        ] {
            let config = SnowflakeConfig {
                account_identifier: account_identifier.to_string(),
                api_key: "jwt".to_string(),
                token_type: SnowflakeTokenType::Oauth,
                endpoint_access: ProviderEndpointAccess::PublicOnly,
                timeout: 60,
                max_retries: 2,
                models: Vec::new(),
            };
            assert!(
                config.api_base().is_err(),
                "account identity must reject {account_identifier}"
            );
        }
    }
}
