//! Strict typed enterprise provider config normalization.

use crate::core::providers::enterprise::EnterpriseProvider;
use crate::core::providers::{ProviderError, ProviderType};

pub(super) async fn build_enterprise_provider(
    provider_type: ProviderType,
    config: serde_json::Value,
) -> Result<EnterpriseProvider, ProviderError> {
    let mut object = config.as_object().cloned().ok_or_else(|| {
        ProviderError::configuration("enterprise", "configuration must be an object")
    })?;
    rename(&mut object, "api_base", "base_url");
    match provider_type {
        ProviderType::Databricks => {
            rename(&mut object, "base_url", "workspace_url");
            let config = serde_json::from_value::<
                crate::core::providers::databricks::DatabricksConfig,
            >(object.into())
            .map_err(|error| ProviderError::configuration("databricks", error.to_string()))?;
            Ok(EnterpriseProvider::Databricks(config.build().await?))
        }
        ProviderType::Snowflake => {
            rename(&mut object, "organization", "account_identifier");
            object.remove("account_id");
            let config =
                serde_json::from_value::<crate::core::providers::snowflake::SnowflakeConfig>(
                    object.into(),
                )
                .map_err(|error| ProviderError::configuration("snowflake", error.to_string()))?;
            Ok(EnterpriseProvider::Snowflake(config.build().await?))
        }
        ProviderType::Oci => {
            if let Some(api_key) = object.remove("api_key") {
                object
                    .entry("auth".to_string())
                    .or_insert_with(|| serde_json::json!({"type":"api_key", "token":api_key}));
            }
            let config =
                serde_json::from_value::<crate::core::providers::oci::OciConfig>(object.into())
                    .map_err(|error| ProviderError::configuration("oci", error.to_string()))?;
            Ok(EnterpriseProvider::Oci(config.build().await?))
        }
        ProviderType::Watsonx => {
            if object.remove("api_key").is_some() {
                return Err(ProviderError::configuration(
                    "watsonx",
                    "api_key is not an IAM access token; configure settings.access_token explicitly",
                ));
            }
            rename(&mut object, "project", "project_id");
            let config = serde_json::from_value::<crate::core::providers::watsonx::WatsonxConfig>(
                object.into(),
            )
            .map_err(|error| ProviderError::configuration("watsonx", error.to_string()))?;
            Ok(EnterpriseProvider::Watsonx(
                crate::core::providers::watsonx::WatsonxProvider::new(config)?,
            ))
        }
        ProviderType::SageMaker => {
            object.remove("api_key");
            let config =
                serde_json::from_value::<crate::core::providers::sagemaker::SageMakerConfig>(
                    object.into(),
                )
                .map_err(|error| ProviderError::configuration("sagemaker", error.to_string()))?;
            Ok(EnterpriseProvider::SageMaker(
                crate::core::providers::sagemaker::SageMakerProvider::new(config)?,
            ))
        }
        _ => Err(ProviderError::configuration(
            "enterprise",
            "not an enterprise provider type",
        )),
    }
}

fn rename(object: &mut serde_json::Map<String, serde_json::Value>, from: &str, to: &str) {
    if let Some(value) = object.remove(from) {
        object.entry(to.to_string()).or_insert(value);
    }
}

#[cfg(test)]
pub(super) fn minimal_test_config(provider_type: &ProviderType) -> Option<serde_json::Value> {
    Some(match provider_type {
        ProviderType::Databricks => {
            serde_json::json!({"workspace_url":"https://dbc.example.com","api_key":"test","timeout":30,"max_retries":2})
        }
        ProviderType::Snowflake => {
            serde_json::json!({"account_identifier":"org-account","api_key":"test","token_type":"OAUTH","timeout":30,"max_retries":2})
        }
        ProviderType::Oci => {
            serde_json::json!({"region":"us-chicago-1","compartment_id":null,"auth":{"type":"api_key","token":"test"},"api_mode":"open_ai_compatible","base_url":null,"timeout":30,"max_retries":2})
        }
        ProviderType::Watsonx => {
            serde_json::json!({"access_token":"test","project_id":"project","space_id":null,"region":"us-south","timeout":30,"max_retries":2})
        }
        ProviderType::SageMaker => {
            serde_json::json!({"aws_access_key_id":"AKIATEST","aws_secret_access_key":"secret","aws_session_token":null,"region":"us-east-1","endpoint_name":"chat-endpoint","payload_transformer":"open_ai_chat","target_model":null,"target_variant":null,"base_url":null,"models":["tenant-chat"],"timeout":30})
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::models::provider::ProviderConfig;
    use crate::core::router::{DefaultRuntimeBinding, RuntimeBinding, UnifiedRouter};
    use std::sync::Arc;

    #[tokio::test]
    async fn failed_refresh_construction_preserves_current_runtime() {
        let binding =
            DefaultRuntimeBinding::new(RuntimeBinding::new(Arc::new(UnifiedRouter::default())));
        let generation = binding.load().generation();
        let invalid = ProviderConfig {
            name: "oci-native".to_string(),
            provider_type: "oci".to_string(),
            models: vec!["cohere.rerank-v3-5".to_string()],
            settings: serde_json::from_value(serde_json::json!({
                "region": "us-chicago-1",
                "compartment_id": "ocid1.compartment.oc1..test",
                "api_mode": "native",
                "auth": {
                    "type": "iam",
                    "tenancy_ocid": "ocid1.tenancy.oc1..test",
                    "user_ocid": "ocid1.user.oc1..test",
                    "fingerprint": "aa:bb:cc",
                    "private_key_pem": "not-an-rsa-private-key"
                }
            }))
            .expect("settings object"),
            ..ProviderConfig::default()
        };

        let construction_failed = match UnifiedRouter::from_gateway_config(&[invalid], None).await {
            Ok(router) => {
                binding.replace(RuntimeBinding::new(Arc::new(router)));
                false
            }
            Err(_) => true,
        };

        assert!(construction_failed);
        assert_eq!(binding.load().generation(), generation);
    }
}
