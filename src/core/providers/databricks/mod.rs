//! Databricks Model Serving OpenAI-compatible contract.

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::ProviderError;
use crate::core::providers::enterprise::{EnterpriseOpenAiProvider, EnterpriseOpenAiSettings};
use serde::{Deserialize, Serialize};

pub type DatabricksProvider = EnterpriseOpenAiProvider;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabricksConfig {
    pub workspace_url: String,
    pub api_key: String,
    #[serde(default)]
    pub endpoint_access: ProviderEndpointAccess,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub models: Vec<String>,
}

impl std::fmt::Debug for DatabricksConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabricksConfig")
            .field("workspace_url", &self.workspace_url)
            .field("api_key", &"[REDACTED]")
            .field("endpoint_access", &self.endpoint_access)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("models", &self.models)
            .finish()
    }
}

impl DatabricksConfig {
    pub fn api_base(&self) -> Result<String, ProviderError> {
        let workspace = self.workspace_url.trim_end_matches('/');
        let parsed = reqwest::Url::parse(workspace).map_err(|error| {
            ProviderError::configuration("databricks", format!("invalid workspace_url: {error}"))
        })?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(ProviderError::configuration(
                "databricks",
                "workspace_url must be an HTTP(S) origin",
            ));
        }
        if parsed.path() != "/" && !parsed.path().is_empty() {
            return Err(ProviderError::configuration(
                "databricks",
                "workspace_url must not contain a path",
            ));
        }
        Ok(format!("{workspace}/serving-endpoints"))
    }

    pub async fn build(self) -> Result<DatabricksProvider, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::configuration(
                "databricks",
                "api_key is required",
            ));
        }
        EnterpriseOpenAiProvider::new(
            "databricks",
            EnterpriseOpenAiSettings {
                api_base: self.api_base()?,
                api_key: self.api_key,
                model_prefix: "databricks/",
                endpoint_access: self.endpoint_access,
                timeout: self.timeout,
                max_retries: self.max_retries,
                headers: Default::default(),
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
    fn constructs_serving_endpoint_base_and_rejects_pathful_workspace() {
        let config = DatabricksConfig {
            workspace_url: "https://dbc.example.cloud.databricks.com".to_string(),
            api_key: "test".to_string(),
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
            models: Vec::new(),
        };
        assert_eq!(
            config.api_base().expect("valid workspace"),
            "https://dbc.example.cloud.databricks.com/serving-endpoints"
        );
        let invalid = DatabricksConfig {
            workspace_url: "https://dbc.example/x".to_string(),
            ..config
        };
        assert!(invalid.api_base().is_err());
    }

    #[tokio::test]
    async fn runtime_identity_and_capabilities_are_platform_specific() {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        use crate::core::types::model::ProviderCapability;
        let provider = DatabricksConfig {
            workspace_url: "https://dbc.example.cloud.databricks.com".to_string(),
            api_key: "test".to_string(),
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
            models: vec!["served-model".to_string()],
        }
        .build()
        .await
        .expect("valid config should build");
        assert_eq!(provider.name(), "databricks");
        assert!(provider.supports_capability(&ProviderCapability::ChatCompletionStream));
        assert!(!provider.supports_capability(&ProviderCapability::Embeddings));
        assert!(provider.supports_model("databricks/served-model"));
        assert!(!provider.supports_model("other-model"));
    }
}
