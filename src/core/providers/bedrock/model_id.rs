//! Bedrock model ID parsing.
//!
//! Bedrock accepts foundation model IDs, inference profile IDs, and ARNs as
//! execution `modelId` values. Only the user-facing `bedrock/` selector is
//! removed from the execution path.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BedrockModelIdKind {
    Arn,
    InferenceProfile,
    FoundationModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeConfigFallback {
    Converse,
    Invoke,
    PromptConverse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBedrockModelId {
    pub user_selector: String,
    pub execution_model_id: String,
    pub metadata_lookup_ids: Vec<String>,
    pub kind: BedrockModelIdKind,
    pub family_hint: Option<String>,
    pub runtime_config_fallback: Option<RuntimeConfigFallback>,
}

impl ParsedBedrockModelId {
    pub fn new(model_id: &str) -> Self {
        let user_selector = model_id.trim().to_string();
        let execution_model_id = user_selector
            .strip_prefix("bedrock/")
            .unwrap_or(&user_selector)
            .to_string();
        let mut metadata_lookup_ids = Vec::new();

        push_unique(&mut metadata_lookup_ids, execution_model_id.clone());

        let mut runtime_config_fallback = None;

        if let Some(resource) = arn_resource_metadata(&execution_model_id) {
            for resource_id in resource.lookup_ids {
                push_unique(&mut metadata_lookup_ids, resource_id.clone());
                if let Some(canonical) = canonical_metadata_id(&resource_id) {
                    push_unique(&mut metadata_lookup_ids, canonical);
                }
            }
            runtime_config_fallback = resource.runtime_config_fallback;
        } else if let Some(canonical) = canonical_metadata_id(&execution_model_id) {
            push_unique(&mut metadata_lookup_ids, canonical);
        }

        let family_hint = metadata_lookup_ids
            .iter()
            .rev()
            .find_map(|id| id.split_once('.').map(|(family, _)| family.to_string()));

        let kind = if execution_model_id.starts_with("arn:") {
            BedrockModelIdKind::Arn
        } else if canonical_metadata_id(&execution_model_id).is_some() {
            BedrockModelIdKind::InferenceProfile
        } else {
            BedrockModelIdKind::FoundationModel
        };

        Self {
            user_selector,
            execution_model_id,
            metadata_lookup_ids,
            kind,
            family_hint,
            runtime_config_fallback,
        }
    }

    pub fn canonical_metadata_id(&self) -> &str {
        self.metadata_lookup_ids
            .last()
            .map(String::as_str)
            .unwrap_or(&self.execution_model_id)
    }
}

pub fn parse_bedrock_model_id(model_id: &str) -> ParsedBedrockModelId {
    ParsedBedrockModelId::new(model_id)
}

pub fn get_model_config_for_model_id(
    model_id: &str,
) -> Result<&'static super::model_config::ModelConfig, crate::core::providers::ProviderError> {
    let parsed = parse_bedrock_model_id(model_id);
    for lookup_id in &parsed.metadata_lookup_ids {
        if let Ok(config) = super::model_config::get_model_config(lookup_id) {
            return Ok(config);
        }
    }

    if let Some(fallback) = parsed.runtime_config_fallback {
        return Ok(match fallback {
            RuntimeConfigFallback::Converse => runtime_resolved_converse_config(),
            RuntimeConfigFallback::Invoke => runtime_resolved_invoke_config(),
            RuntimeConfigFallback::PromptConverse => runtime_resolved_prompt_config(),
        });
    }

    Err(crate::core::providers::ProviderError::model_not_found(
        "bedrock",
        format!("Model {} not supported", parsed.execution_model_id),
    ))
}

static RUNTIME_RESOLVED_CONVERSE_CONFIG: super::model_config::ModelConfig =
    super::model_config::ModelConfig {
        family: super::model_config::BedrockModelFamily::Nova,
        api_type: super::model_config::BedrockApiType::ConverseStream,
        supports_streaming: true,
        supports_function_calling: true,
        supports_multimodal: true,
        max_context_length: 0,
        max_output_length: None,
        input_cost_per_1k: 0.0,
        output_cost_per_1k: 0.0,
    };

static RUNTIME_RESOLVED_INVOKE_CONFIG: super::model_config::ModelConfig =
    super::model_config::ModelConfig {
        family: super::model_config::BedrockModelFamily::DeepSeek,
        api_type: super::model_config::BedrockApiType::InvokeStream,
        supports_streaming: true,
        supports_function_calling: false,
        supports_multimodal: false,
        max_context_length: 0,
        max_output_length: None,
        input_cost_per_1k: 0.0,
        output_cost_per_1k: 0.0,
    };

static RUNTIME_RESOLVED_PROMPT_CONFIG: super::model_config::ModelConfig =
    super::model_config::ModelConfig {
        family: super::model_config::BedrockModelFamily::Nova,
        api_type: super::model_config::BedrockApiType::ConverseStream,
        supports_streaming: true,
        supports_function_calling: false,
        supports_multimodal: false,
        max_context_length: 0,
        max_output_length: None,
        input_cost_per_1k: 0.0,
        output_cost_per_1k: 0.0,
    };

fn runtime_resolved_converse_config() -> &'static super::model_config::ModelConfig {
    &RUNTIME_RESOLVED_CONVERSE_CONFIG
}

fn runtime_resolved_invoke_config() -> &'static super::model_config::ModelConfig {
    &RUNTIME_RESOLVED_INVOKE_CONFIG
}

fn runtime_resolved_prompt_config() -> &'static super::model_config::ModelConfig {
    &RUNTIME_RESOLVED_PROMPT_CONFIG
}

fn canonical_metadata_id(model_id: &str) -> Option<String> {
    let (prefix, rest) = model_id.split_once('.')?;
    if is_geo_prefix(prefix) || is_region_prefix(prefix) {
        Some(rest.to_string())
    } else {
        None
    }
}

#[derive(Debug, Default)]
struct ArnResourceMetadata {
    lookup_ids: Vec<String>,
    runtime_config_fallback: Option<RuntimeConfigFallback>,
}

fn arn_resource_parts(model_id: &str) -> Option<(&str, &str)> {
    if !model_id.starts_with("arn:") {
        return None;
    }

    let resource = model_id.splitn(6, ':').nth(5)?;
    resource.split_once('/')
}

fn is_prompt_management_resource(resource_type: &str) -> bool {
    matches!(
        resource_type,
        "prompt" | "default-prompt-router" | "prompt-router"
    )
}

pub(in crate::core::providers::bedrock) fn is_prompt_management_model_id(model_id: &str) -> bool {
    let execution_model_id = model_id.strip_prefix("bedrock/").unwrap_or(model_id);
    arn_resource_parts(execution_model_id)
        .map(|(resource_type, _resource_id)| is_prompt_management_resource(resource_type))
        .unwrap_or(false)
}

pub(in crate::core::providers::bedrock) fn is_runtime_resolved_invoke_model_id(
    model_id: &str,
) -> bool {
    parse_bedrock_model_id(model_id).runtime_config_fallback == Some(RuntimeConfigFallback::Invoke)
}

fn arn_resource_metadata(model_id: &str) -> Option<ArnResourceMetadata> {
    let (resource_type, resource_id) = arn_resource_parts(model_id)?;

    if is_prompt_management_resource(resource_type) {
        return Some(ArnResourceMetadata {
            runtime_config_fallback: Some(RuntimeConfigFallback::PromptConverse),
            ..Default::default()
        });
    }

    if resource_id.is_empty() {
        return Some(ArnResourceMetadata {
            runtime_config_fallback: Some(RuntimeConfigFallback::Invoke),
            ..Default::default()
        });
    }

    let mut metadata = ArnResourceMetadata::default();

    match resource_type {
        "foundation-model" | "inference-profile" => {
            metadata.lookup_ids.push(resource_id.to_string());
        }
        "application-inference-profile" => {
            metadata.lookup_ids.push(resource_id.to_string());
            metadata.runtime_config_fallback = Some(RuntimeConfigFallback::Converse);
        }
        "custom-model" => {
            if let Some((source_model_id, _custom_model_id)) = resource_id.split_once('/')
                && source_model_id != "imported"
            {
                metadata.lookup_ids.push(source_model_id.to_string());
            } else {
                metadata.runtime_config_fallback = Some(RuntimeConfigFallback::Invoke);
            }
        }
        "custom-model-deployment" | "provisioned-model" => {
            metadata.runtime_config_fallback = Some(RuntimeConfigFallback::Converse);
        }
        "imported-model" => {
            metadata.runtime_config_fallback = Some(RuntimeConfigFallback::Invoke);
        }
        _ => {
            metadata.runtime_config_fallback = Some(RuntimeConfigFallback::Invoke);
        }
    }

    Some(metadata)
}

fn is_geo_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "global" | "us" | "eu" | "ap" | "apac" | "sa" | "ca" | "me" | "af" | "jp" | "au"
    )
}

fn is_region_prefix(prefix: &str) -> bool {
    prefix.len() >= 4
        && prefix.contains('-')
        && prefix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::{BedrockModelIdKind, RuntimeConfigFallback, parse_bedrock_model_id};

    fn config_for(model_id: &str) -> &'static crate::core::providers::bedrock::ModelConfig {
        match super::get_model_config_for_model_id(model_id) {
            Ok(config) => config,
            Err(err) => panic!("expected Bedrock config for {model_id}: {err}"),
        }
    }

    #[test]
    fn bedrock_selector_is_removed_only_for_execution() {
        let parsed = parse_bedrock_model_id("bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0");

        assert_eq!(
            parsed.execution_model_id,
            "us.anthropic.claude-3-5-sonnet-20241022-v2:0"
        );
        assert_eq!(
            parsed.metadata_lookup_ids,
            vec![
                "us.anthropic.claude-3-5-sonnet-20241022-v2:0",
                "anthropic.claude-3-5-sonnet-20241022-v2:0"
            ]
        );
        assert_eq!(parsed.kind, BedrockModelIdKind::InferenceProfile);
        assert_eq!(parsed.family_hint.as_deref(), Some("anthropic"));
    }

    #[test]
    fn global_profile_is_preserved_for_execution() {
        let parsed = parse_bedrock_model_id("global.anthropic.claude-sonnet-4-v1:0");

        assert_eq!(
            parsed.execution_model_id,
            "global.anthropic.claude-sonnet-4-v1:0"
        );
        assert_eq!(
            parsed.metadata_lookup_ids,
            vec![
                "global.anthropic.claude-sonnet-4-v1:0",
                "anthropic.claude-sonnet-4-v1:0"
            ]
        );
        assert_eq!(parsed.family_hint.as_deref(), Some("anthropic"));
    }

    #[test]
    fn jp_and_au_geo_profiles_use_base_model_for_metadata() {
        for model_id in [
            "jp.anthropic.claude-3-5-sonnet-20241022-v2:0",
            "au.anthropic.claude-3-5-sonnet-20241022-v2:0",
        ] {
            let parsed = parse_bedrock_model_id(model_id);

            assert_eq!(parsed.execution_model_id, model_id);
            assert_eq!(
                parsed.metadata_lookup_ids,
                vec![model_id, "anthropic.claude-3-5-sonnet-20241022-v2:0"]
            );
            assert_eq!(parsed.kind, BedrockModelIdKind::InferenceProfile);
            assert_eq!(parsed.family_hint.as_deref(), Some("anthropic"));
        }
    }

    #[test]
    fn region_like_model_id_is_preserved_for_execution() {
        let parsed = parse_bedrock_model_id("us-east-1.anthropic.claude-3-haiku-20240307");

        assert_eq!(
            parsed.execution_model_id,
            "us-east-1.anthropic.claude-3-haiku-20240307"
        );
        assert_eq!(
            parsed.metadata_lookup_ids,
            vec![
                "us-east-1.anthropic.claude-3-haiku-20240307",
                "anthropic.claude-3-haiku-20240307"
            ]
        );
        assert_eq!(parsed.family_hint.as_deref(), Some("anthropic"));
    }

    #[test]
    fn arn_is_preserved_for_execution_with_resource_metadata_fallback() {
        let parsed = parse_bedrock_model_id(
            "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.anthropic.claude-3-5-sonnet-20241022-v2:0",
        );

        assert!(parsed.execution_model_id.starts_with("arn:aws:bedrock:"));
        assert_eq!(
            parsed.metadata_lookup_ids,
            vec![
                "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.anthropic.claude-3-5-sonnet-20241022-v2:0",
                "us.anthropic.claude-3-5-sonnet-20241022-v2:0",
                "anthropic.claude-3-5-sonnet-20241022-v2:0"
            ]
        );
        assert_eq!(parsed.kind, BedrockModelIdKind::Arn);
        assert_eq!(parsed.family_hint.as_deref(), Some("anthropic"));
        assert_eq!(parsed.runtime_config_fallback, None);
    }

    #[test]
    fn foundation_model_arn_uses_resource_model_id_for_metadata() {
        let parsed = parse_bedrock_model_id(
            "arn:aws:bedrock:us-east-1::foundation-model/anthropic.claude-3-haiku-20240307",
        );

        assert_eq!(
            parsed.metadata_lookup_ids,
            vec![
                "arn:aws:bedrock:us-east-1::foundation-model/anthropic.claude-3-haiku-20240307",
                "anthropic.claude-3-haiku-20240307"
            ]
        );
        assert_eq!(parsed.family_hint.as_deref(), Some("anthropic"));
        assert_eq!(parsed.runtime_config_fallback, None);
    }

    #[test]
    fn custom_model_arn_uses_source_model_for_metadata() {
        let parsed = parse_bedrock_model_id(
            "arn:aws:bedrock:us-east-1:123456789012:custom-model/anthropic.claude-3-5-sonnet-20241022-v2:0/123456789012",
        );

        assert_eq!(
            parsed.metadata_lookup_ids,
            vec![
                "arn:aws:bedrock:us-east-1:123456789012:custom-model/anthropic.claude-3-5-sonnet-20241022-v2:0/123456789012",
                "anthropic.claude-3-5-sonnet-20241022-v2:0"
            ]
        );
        assert_eq!(parsed.family_hint.as_deref(), Some("anthropic"));
        assert_eq!(parsed.runtime_config_fallback, None);
    }

    #[test]
    fn application_profile_arn_allows_runtime_config_resolution() {
        let parsed = parse_bedrock_model_id(
            "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-team-profile",
        );

        assert_eq!(
            parsed.metadata_lookup_ids,
            vec![
                "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-team-profile",
                "my-team-profile"
            ]
        );
        assert_eq!(
            parsed.runtime_config_fallback,
            Some(RuntimeConfigFallback::Converse)
        );
    }

    #[test]
    fn custom_model_deployment_arn_allows_runtime_config_resolution() {
        let parsed = parse_bedrock_model_id(
            "arn:aws:bedrock:us-east-1:123456789012:custom-model-deployment/123456789012",
        );

        assert_eq!(
            parsed.metadata_lookup_ids,
            vec!["arn:aws:bedrock:us-east-1:123456789012:custom-model-deployment/123456789012"]
        );
        assert_eq!(
            parsed.runtime_config_fallback,
            Some(RuntimeConfigFallback::Converse)
        );
    }

    #[test]
    fn provisioned_model_arn_allows_converse_runtime_config_resolution() {
        let parsed = parse_bedrock_model_id(
            "arn:aws:bedrock:us-east-1:123456789012:provisioned-model/ABC123",
        );

        assert_eq!(
            parsed.metadata_lookup_ids,
            vec!["arn:aws:bedrock:us-east-1:123456789012:provisioned-model/ABC123"]
        );
        assert_eq!(
            parsed.runtime_config_fallback,
            Some(RuntimeConfigFallback::Converse)
        );
    }

    #[test]
    fn prompt_management_arns_use_prompt_converse_fallback() {
        for model_id in [
            "arn:aws:bedrock:us-east-1:123456789012:prompt/ABC123:1",
            "arn:aws:bedrock:us-east-1:123456789012:default-prompt-router/ABC123",
            "arn:aws:bedrock:us-east-1:123456789012:prompt-router/ABC123",
        ] {
            let parsed = parse_bedrock_model_id(model_id);

            assert_eq!(parsed.metadata_lookup_ids, vec![model_id]);
            assert_eq!(
                parsed.runtime_config_fallback,
                Some(RuntimeConfigFallback::PromptConverse)
            );
        }
    }

    #[test]
    fn prompt_management_arn_config_uses_prompt_converse_config() {
        let config = config_for("arn:aws:bedrock:us-east-1:123456789012:prompt/ABC123:1");

        assert_eq!(
            config.api_type,
            crate::core::providers::bedrock::BedrockApiType::ConverseStream
        );
        assert!(config.supports_streaming);
        assert!(!config.supports_function_calling);
    }

    #[test]
    fn application_profile_arn_uses_streaming_converse_config() {
        let config = config_for(
            "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-team-profile",
        );

        assert_eq!(
            config.api_type,
            crate::core::providers::bedrock::BedrockApiType::ConverseStream
        );
        assert!(config.supports_streaming);
    }

    #[test]
    fn imported_model_arn_uses_invoke_compatible_config() {
        let config = config_for("arn:aws:bedrock:us-east-1:123456789012:imported-model/ABC123");

        assert_eq!(
            config.api_type,
            crate::core::providers::bedrock::BedrockApiType::InvokeStream
        );
        assert_eq!(
            config.family,
            crate::core::providers::bedrock::BedrockModelFamily::DeepSeek
        );
        assert!(config.supports_streaming);
        assert!(!config.supports_function_calling);
    }

    #[test]
    fn unknown_runtime_resolved_arn_uses_invoke_compatible_config() {
        let config = config_for("arn:aws:bedrock:us-east-1:123456789012:unknown-resource/ABC123");

        assert_eq!(
            config.api_type,
            crate::core::providers::bedrock::BedrockApiType::InvokeStream
        );
    }
}
