use super::AzureAIModelRegistry;
use crate::core::providers::model_identity::DeploymentModelIdentity;
use crate::core::types::model::ProviderCapability;

const CHAT_PARAMS: &[&str] = &[
    "temperature",
    "max_tokens",
    "max_completion_tokens",
    "top_p",
    "frequency_penalty",
    "presence_penalty",
];
const CHAT_STREAM_PARAMS: &[&str] = &[
    "temperature",
    "max_tokens",
    "max_completion_tokens",
    "top_p",
    "frequency_penalty",
    "presence_penalty",
    "stream",
];
const CHAT_TOOL_PARAMS: &[&str] = &[
    "temperature",
    "max_tokens",
    "max_completion_tokens",
    "top_p",
    "frequency_penalty",
    "presence_penalty",
    "tools",
    "tool_choice",
];
const CHAT_TOOL_STREAM_PARAMS: &[&str] = &[
    "temperature",
    "max_tokens",
    "max_completion_tokens",
    "top_p",
    "frequency_penalty",
    "presence_penalty",
    "tools",
    "tool_choice",
    "stream",
];

pub(super) fn supported_openai_params(
    registry: &AzureAIModelRegistry,
    identity: Option<&DeploymentModelIdentity>,
    wire_model: &str,
) -> &'static [&'static str] {
    let features = match identity {
        Some(identity) => match (
            identity.capability_catalog_provider(),
            identity.capability_catalog_model(),
        ) {
            (Some("openai"), Some(model)) => openai_chat_features(model),
            (Some("azure_ai"), Some(model)) => azure_ai_chat_features(registry, model),
            _ => None,
        },
        None => azure_ai_chat_features(registry, wire_model),
    };

    match features {
        Some((false, false)) => CHAT_PARAMS,
        Some((false, true)) => CHAT_STREAM_PARAMS,
        Some((true, false)) => CHAT_TOOL_PARAMS,
        Some((true, true)) => CHAT_TOOL_STREAM_PARAMS,
        None => &[],
    }
}

fn azure_ai_chat_features(registry: &AzureAIModelRegistry, model: &str) -> Option<(bool, bool)> {
    registry
        .get_model(model)
        .filter(|model| {
            model
                .capabilities
                .contains(&ProviderCapability::ChatCompletion)
        })
        .map(|model| (model.supports_function_calling, model.supports_streaming))
}

fn openai_chat_features(model: &str) -> Option<(bool, bool)> {
    crate::core::providers::openai::models::get_openai_registry()
        .get_model_spec(model)
        .filter(|model| {
            model
                .model_info
                .capabilities
                .contains(&ProviderCapability::ChatCompletion)
        })
        .map(|model| {
            let capabilities = &model.model_info.capabilities;
            (
                capabilities.contains(&ProviderCapability::ToolCalling),
                capabilities.contains(&ProviderCapability::ChatCompletionStream),
            )
        })
}
