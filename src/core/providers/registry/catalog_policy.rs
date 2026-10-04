//! Provider-scoped compatibility policy for catalog-backed native duplicates.

use std::collections::HashMap;

use serde_json::Value;

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::model::ModelInfo;

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
pub(crate) fn catalog_model_infos(_provider: &str) -> Option<&'static [ModelInfo]> {
    None
}

pub(crate) fn catalog_model_info(_provider: &str, _model_id: &str) -> Option<ModelInfo> {
    None
}

pub(crate) fn catalog_provider_supports_model(_provider: &str, _model_id: &str) -> Option<bool> {
    None
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
    provider == "meta_llama"
}

pub(crate) fn organization_header(provider: &str) -> &'static str {
    match provider {
        "meta_llama" => "X-Organization-ID",
        _ => "OpenAI-Organization",
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unverified_catalogs_have_no_static_callable_models() {
        for provider in ["meta_llama", "amazon_nova", "v0", "github"] {
            assert!(catalog_model_infos(provider).is_none());
            assert!(catalog_model_info(provider, "v0-default").is_none());
        }
    }
    #[test]
    fn unscoped_catalog_params_remain_passthrough() {
        let params = HashMap::from([("provider_extension".to_string(), Value::from(true))]);
        assert_eq!(filter_openai_params("openrouter", params.clone()), params);
    }
}
