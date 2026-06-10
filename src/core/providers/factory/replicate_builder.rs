//! Replicate provider config builder.

use super::builder::{
    config_bool, config_str, config_str_any, config_u32, config_u64, env_str_any,
    merge_string_headers,
};
use crate::core::providers::{replicate, unified_provider::ProviderError};
use crate::core::traits::provider::ProviderConfig as _;

pub(super) fn build_replicate_config_from_factory(
    config: &serde_json::Value,
) -> Result<replicate::ReplicateConfig, ProviderError> {
    reject_unsupported_fields(config)?;

    let api_key = config_str_any(config, &["api_key", "api_token"])
        .map(str::to_string)
        .or_else(|| env_str_any(&["REPLICATE_API_TOKEN", "REPLICATE_API_KEY"]))
        .ok_or_else(|| {
            ProviderError::configuration("replicate", "api_key or REPLICATE_API_TOKEN is required")
        })?;

    let mut replicate_config = replicate::ReplicateConfig::new(api_key);

    if let Some(api_base) =
        config_str(config, "base_url").or_else(|| config_str(config, "api_base"))
    {
        replicate_config.base.api_base = Some(api_base.to_string());
    }
    if let Some(timeout) =
        config_u64(config, "timeout_seconds").or_else(|| config_u64(config, "timeout"))
    {
        replicate_config.base.timeout = timeout;
    }
    if let Some(max_retries) = config_u32(config, "max_retries") {
        replicate_config.base.max_retries = max_retries;
    }
    if let Some(polling_delay) = config_u64(config, "polling_delay_seconds") {
        replicate_config.polling_delay_seconds = polling_delay;
    }
    if let Some(polling_retries) = config_u32(config, "polling_retries") {
        replicate_config.polling_retries = polling_retries;
    }
    if let Some(use_streaming) = config_bool(config, "use_streaming") {
        replicate_config.use_streaming = use_streaming;
    }
    merge_string_headers(&mut replicate_config.base.headers, config, "headers");
    merge_string_headers(&mut replicate_config.base.headers, config, "custom_headers");

    replicate_config
        .validate()
        .map_err(|err| ProviderError::configuration("replicate", err))?;
    Ok(replicate_config)
}

fn reject_unsupported_fields(config: &serde_json::Value) -> Result<(), ProviderError> {
    for field in ["api_version", "organization", "account_id", "project"] {
        if config_str(config, field).is_some() {
            return Err(ProviderError::invalid_request(
                "replicate",
                format!("Replicate native dispatch does not support `{field}`"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_replicate_config_accepts_aliases_and_headers() {
        let config_result = build_replicate_config_from_factory(&serde_json::json!({
            "api_token": "test-token",
            "api_base": "https://replicate.example/v1",
            "timeout_seconds": 45,
            "max_retries": 4,
            "polling_delay_seconds": 2,
            "polling_retries": 5,
            "use_streaming": true,
            "headers": {"x-replicate-header": "one"},
            "custom_headers": {"x-custom-header": "two"}
        }));
        let config = match config_result {
            Ok(config) => config,
            Err(err) => panic!("replicate config should accept supported native fields: {err}"),
        };

        assert_eq!(config.base.api_key.as_deref(), Some("test-token"));
        assert_eq!(config.get_api_base(), "https://replicate.example/v1");
        assert_eq!(config.base.timeout, 45);
        assert_eq!(config.base.max_retries, 4);
        assert_eq!(config.polling_delay_seconds, 2);
        assert_eq!(config.polling_retries, 5);
        assert!(config.use_streaming);
        assert_eq!(
            config
                .base
                .headers
                .get("x-replicate-header")
                .map(String::as_str),
            Some("one")
        );
        assert_eq!(
            config
                .base
                .headers
                .get("x-custom-header")
                .map(String::as_str),
            Some("two")
        );
    }

    #[test]
    fn test_build_replicate_config_rejects_unsupported_fields() {
        let result = build_replicate_config_from_factory(&serde_json::json!({
            "api_key": "test-token",
            "project": "unused-project"
        }));
        let err = match result {
            Ok(_) => panic!("replicate should fail fast on unsupported project field"),
            Err(err) => err,
        };

        assert!(
            matches!(err, ProviderError::InvalidRequest { .. }),
            "expected InvalidRequest, got {err}"
        );
        assert!(
            err.to_string().contains("project"),
            "error should identify unsupported field: {err}"
        );
    }
}
