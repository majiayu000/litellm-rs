//! Provider-scoped compatibility policy for catalog-backed native duplicates.

use std::{collections::HashMap, sync::LazyLock};

use serde_json::Value;

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::model::{ModelInfo, ProviderCapability};

pub(crate) const META_LLAMA_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];
pub(crate) const V0_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::FunctionCalling,
];

const COMMON_OPENAI_PARAMS: &[&str] = &[
    "messages",
    "model",
    "temperature",
    "max_tokens",
    "max_completion_tokens",
    "top_p",
    "frequency_penalty",
    "presence_penalty",
    "stop",
    "stream",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
    "response_format",
    "user",
    "seed",
    "n",
    "logit_bias",
    "logprobs",
    "top_logprobs",
    "reasoning_effort",
    "store",
    "metadata",
    "service_tier",
];
const META_LLAMA_OPENAI_PARAMS: &[&str] = &[
    "messages",
    "model",
    "max_tokens",
    "temperature",
    "top_p",
    "n",
    "stream",
    "stop",
    "presence_penalty",
    "frequency_penalty",
    "user",
    "seed",
    "response_format",
    "tools",
    "tool_choice",
];
const V0_OPENAI_PARAMS: &[&str] = &[
    "messages",
    "model",
    "temperature",
    "max_tokens",
    "top_p",
    "stream",
    "tools",
    "tool_choice",
    "functions",
    "function_call",
    "user",
    "seed",
];
const META_LLAMA_SUPPORTED_MODELS: &[&str] = &[
    "llama4-scout",
    "llama4-maverick",
    "llama3.3-70b",
    "llama3.2-1b",
    "llama3.2-3b",
    "llama3.2-11b-vision",
    "llama3.2-90b-vision",
    "llama3.1-8b",
    "llama3.1-70b",
    "llama3.1-405b",
];

struct CatalogModel {
    id: &'static str,
    name: &'static str,
    provider: &'static str,
    context: u32,
    output: Option<u32>,
    multimodal: bool,
    input_cost: f64,
    output_cost: f64,
}

const META_LLAMA_MODELS: &[CatalogModel] = &[
    catalog_model(
        "llama4-scout",
        "Llama 4 Scout",
        "meta",
        10_000_000,
        Some(128_000),
        true,
        0.00008,
        0.0003,
    ),
    catalog_model(
        "llama4-maverick",
        "Llama 4 Maverick",
        "meta",
        1_000_000,
        Some(128_000),
        true,
        0.00020,
        0.0006,
    ),
    catalog_model(
        "llama3.3-70b",
        "Llama 3.3 70B",
        "meta",
        128_000,
        Some(32_000),
        false,
        0.0006,
        0.0006,
    ),
    catalog_model(
        "llama3.1-405b",
        "Llama 3.1 405B",
        "meta",
        128_000,
        None,
        false,
        0.002,
        0.002,
    ),
    catalog_model(
        "llama3.1-70b",
        "Llama 3.1 70B",
        "meta",
        128_000,
        None,
        false,
        0.001,
        0.001,
    ),
];
const V0_MODELS: &[CatalogModel] = &[catalog_model(
    "v0-default",
    "V0 Default Model",
    "v0",
    32_768,
    Some(8_192),
    false,
    0.1,
    0.2,
)];

static META_LLAMA_MODEL_INFOS: LazyLock<Vec<ModelInfo>> = LazyLock::new(|| {
    META_LLAMA_MODELS
        .iter()
        .map(catalog_model_info_from_entry)
        .collect()
});
static V0_MODEL_INFOS: LazyLock<Vec<ModelInfo>> = LazyLock::new(|| {
    let canonical = catalog_model_info_from_entry(&V0_MODELS[0]);
    let mut alias = canonical.clone();
    alias.id = "v0".to_string();
    vec![canonical, alias]
});

#[allow(clippy::too_many_arguments)]
const fn catalog_model(
    id: &'static str,
    name: &'static str,
    provider: &'static str,
    context: u32,
    output: Option<u32>,
    multimodal: bool,
    input_cost: f64,
    output_cost: f64,
) -> CatalogModel {
    CatalogModel {
        id,
        name,
        provider,
        context,
        output,
        multimodal,
        input_cost,
        output_cost,
    }
}

fn catalog_model_info_from_entry(model: &CatalogModel) -> ModelInfo {
    let capabilities = if model.provider == "v0" {
        V0_CAPABILITIES.to_vec()
    } else {
        META_LLAMA_CAPABILITIES.to_vec()
    };
    ModelInfo {
        id: model.id.to_string(),
        name: model.name.to_string(),
        provider: model.provider.to_string(),
        max_context_length: model.context,
        max_output_length: model.output,
        supports_streaming: true,
        supports_tools: true,
        supports_multimodal: model.multimodal,
        input_cost_per_1k_tokens: Some(model.input_cost),
        output_cost_per_1k_tokens: Some(model.output_cost),
        currency: "USD".to_string(),
        capabilities,
        ..Default::default()
    }
}

pub(crate) fn catalog_model_infos(provider: &str) -> Option<&'static [ModelInfo]> {
    match provider {
        "amazon_nova" => Some(super::catalog::amazon_nova_catalog_model_infos()),
        "github" => Some(super::github_policy::github_catalog_model_infos()),
        "meta_llama" => Some(&META_LLAMA_MODEL_INFOS),
        "v0" => Some(&V0_MODEL_INFOS),
        _ => None,
    }
}

pub(crate) fn catalog_model_info(provider: &str, model_id: &str) -> Option<ModelInfo> {
    match provider {
        "amazon_nova" => super::catalog::amazon_nova_catalog_model_info(model_id),
        "github" => super::github_policy::github_catalog_model_info(model_id),
        "meta_llama" => {
            find_catalog_model(META_LLAMA_MODELS, model_id).map(catalog_model_info_from_entry)
        }
        "v0" => find_catalog_model(V0_MODELS, v0_canonical_model(model_id))
            .map(catalog_model_info_from_entry),
        _ => None,
    }
}

pub(crate) fn catalog_provider_supports_model(provider: &str, model_id: &str) -> Option<bool> {
    match provider {
        "meta_llama" => Some(
            META_LLAMA_SUPPORTED_MODELS
                .iter()
                .any(|supported| model_id == *supported || model_id.contains(supported)),
        ),
        "v0" => Some(true),
        _ => None,
    }
}

pub(crate) fn catalog_provider_supported_openai_params(provider: &str) -> &'static [&'static str] {
    match provider {
        "meta_llama" => META_LLAMA_OPENAI_PARAMS,
        "v0" => V0_OPENAI_PARAMS,
        _ => COMMON_OPENAI_PARAMS,
    }
}

pub(crate) fn filter_openai_params(
    provider: &str,
    params: HashMap<String, Value>,
) -> HashMap<String, Value> {
    if !matches!(provider, "meta_llama" | "v0") {
        return params;
    }
    let supported = catalog_provider_supported_openai_params(provider);
    params
        .into_iter()
        .filter(|(key, value)| {
            supported.contains(&key.as_str())
                && (provider != "meta_llama"
                    || key != "response_format"
                    || value.get("type").and_then(Value::as_str) == Some("json_schema"))
        })
        .collect()
}

pub(crate) fn filter_request(provider: &str, request: &mut Value) {
    if !matches!(provider, "meta_llama" | "v0") {
        return;
    }
    let supported = catalog_provider_supported_openai_params(provider);
    if let Some(request) = request.as_object_mut() {
        request.retain(|key, value| {
            supported.contains(&key.as_str())
                && (provider != "meta_llama"
                    || key != "response_format"
                    || value.get("type").and_then(Value::as_str) == Some("json_schema"))
        });
    }
}

pub(crate) fn health_failure_is_unhealthy(provider: &str) -> bool {
    provider == "v0"
}

pub(crate) fn preserves_configured_name_route(provider: &str) -> bool {
    matches!(provider, "meta_llama" | "v0")
}

pub(crate) fn catalog_error_response(
    provider: &str,
    status: u16,
    body: &str,
) -> Option<ProviderError> {
    match provider {
        "meta_llama" => Some(HttpErrorMapper::map_status_code("meta_llama", status, body)),
        "v0" => Some(HttpErrorMapper::map_status_code("v0", status, body)),
        _ => None,
    }
}

fn find_catalog_model(models: &'static [CatalogModel], id: &str) -> Option<&'static CatalogModel> {
    models.iter().find(|model| model.id == id)
}

fn v0_canonical_model(model: &str) -> &str {
    if model == "v0" { "v0-default" } else { model }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::provider::ProviderConfig;
    use crate::core::providers::openai_like::OpenAILikeProvider;
    use crate::core::providers::registry::catalog::get_definition;
    use crate::core::router::unified::Router;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use crate::core::types::chat::ChatRequest;
    use crate::core::types::context::RequestContext;
    use crate::core::types::tools::ResponseFormat;

    #[tokio::test]
    async fn meta_llama_catalog_runtime_preserves_models_and_filtering() {
        let models = catalog_model_infos("meta_llama").expect("Meta catalog models");
        assert_eq!(models.len(), 5);
        assert_eq!(models[0].provider, "meta");
        assert_eq!(models[0].max_context_length, 10_000_000);
        assert_eq!(models[0].input_cost_per_1k_tokens, Some(0.00008));
        assert!(
            catalog_provider_supports_model("meta_llama", "meta_llama/llama3.2-11b-vision")
                == Some(true)
        );
        assert!(catalog_provider_supports_model("meta_llama", "gpt-4") == Some(false));

        let filtered = filter_openai_params(
            "meta_llama",
            HashMap::from([("service_tier".to_string(), Value::from("flex"))]),
        );
        assert!(!filtered.contains_key("service_tier"));

        let definition = get_definition("meta_llama").expect("Meta catalog definition");
        let provider = OpenAILikeProvider::new_for_catalog(
            definition
                .to_openai_like_config(Some("test-key"), None)
                .with_provider_name("meta_llama"),
            definition.capabilities,
        )
        .await
        .expect("Meta catalog runtime");
        assert_eq!(provider.models().len(), 5);
        assert!(!provider.supports_model("gpt-4"));
        let request = ChatRequest {
            model: "llama4-scout".to_string(),
            service_tier: Some("flex".to_string()),
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
                response_type: None,
            }),
            ..Default::default()
        };
        let body = provider
            .transform_request(request, RequestContext::default())
            .await
            .expect("Meta request transforms");
        assert!(body.get("service_tier").is_none());
        assert!(body.get("response_format").is_none());
    }

    #[tokio::test]
    async fn v0_catalog_runtime_preserves_alias_metadata_pricing_and_capabilities() {
        let canonical = catalog_model_info("v0", "v0-default").expect("canonical V0 model");
        let alias = catalog_model_info("v0", "v0").expect("V0 alias");
        assert_eq!(canonical.id, alias.id);
        assert_eq!(canonical.max_context_length, 32_768);
        assert_eq!(canonical.max_output_length, Some(8_192));
        assert_eq!(canonical.input_cost_per_1k_tokens, Some(0.1));
        assert_eq!(canonical.output_cost_per_1k_tokens, Some(0.2));
        assert_eq!(V0_CAPABILITIES.len(), 4);
        assert!(
            canonical
                .capabilities
                .contains(&ProviderCapability::FunctionCalling)
        );
        assert!(health_failure_is_unhealthy("v0"));
        assert!(matches!(
            catalog_error_response("v0", 404, r#"{"error":{"message":"missing"}}"#),
            Some(ProviderError::ModelNotFound { provider, .. }) if provider == "v0"
        ));
        assert!(matches!(
            catalog_error_response("meta_llama", 408, "request timeout"),
            Some(ProviderError::Timeout { provider, .. }) if provider == "meta_llama"
        ));

        let definition = get_definition("v0").expect("V0 catalog definition");
        let provider = OpenAILikeProvider::new_for_catalog(
            definition
                .to_openai_like_config(Some("test-key"), None)
                .with_provider_name("v0"),
            definition.capabilities,
        )
        .await
        .expect("V0 catalog runtime");
        assert_eq!(provider.models().len(), 2);
        assert!(
            provider
                .models()
                .iter()
                .any(|model| model.id == canonical.id)
        );
        assert!(provider.models().iter().any(|model| model.id == "v0"));
        assert_eq!(provider.get_model_info("v0").id, "v0-default");
        let cost = provider
            .calculate_cost("v0", 1_000, 1_000)
            .await
            .expect("V0 catalog pricing");
        assert!((cost - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn unscoped_catalog_params_remain_passthrough() {
        let params = HashMap::from([("provider_extension".to_string(), Value::from(true))]);
        assert_eq!(filter_openai_params("openrouter", params.clone()), params);
    }

    #[tokio::test]
    async fn v0_catalog_registers_canonical_and_provider_alias_routes() {
        let router = Router::from_gateway_config(
            &[ProviderConfig {
                name: "frontend".to_string(),
                provider_type: "v0".to_string(),
                api_key: "test-key".to_string(),
                ..ProviderConfig::default()
            }],
            None,
        )
        .await
        .expect("V0 router should build");

        let models = router.list_models();
        assert!(models.contains(&"v0-default".to_string()));
        assert!(models.contains(&"v0".to_string()));
        assert!(models.contains(&"frontend".to_string()));

        let meta_router = Router::from_gateway_config(
            &[ProviderConfig {
                name: "llama_frontend".to_string(),
                provider_type: "meta_llama".to_string(),
                api_key: "test-key".to_string(),
                ..ProviderConfig::default()
            }],
            None,
        )
        .await
        .expect("Meta router should build");
        assert!(
            meta_router
                .list_models()
                .contains(&"llama_frontend".to_string())
        );
    }
}
