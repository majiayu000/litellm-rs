use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::HttpRequest;

pub(super) fn attach_api_key_token_limit(
    req: &HttpRequest,
    context: &mut RequestContext,
) -> Result<(), GatewayError> {
    if let Some(limit) = super::context::api_key_max_tokens_per_request(req)? {
        context.set_api_key_max_tokens_per_request(limit);
    }
    Ok(())
}

pub(super) fn requested_chat_output_token_limit(request: &ChatCompletionRequest) -> Option<u32> {
    requested_output_token_limit(request.max_tokens, request.max_completion_tokens)
}

pub(super) fn requested_output_token_limit(
    max_tokens: Option<u32>,
    max_completion_tokens: Option<u32>,
) -> Option<u32> {
    max_tokens.into_iter().chain(max_completion_tokens).max()
}

pub(super) fn apply_api_key_output_token_limit(
    max_tokens_per_request: Option<u32>,
    provider: &str,
    model: &str,
    request: &mut ChatRequest,
) -> Result<(), ProviderError> {
    let Some(limit) = max_tokens_per_request else {
        return Ok(());
    };

    if let Some(requested) =
        requested_output_token_limit(request.max_tokens, request.max_completion_tokens)
        && requested > limit
    {
        return Err(token_policy_error(requested, limit));
    }

    if request.max_tokens.is_none() {
        request.max_tokens = request.max_completion_tokens.or(Some(limit));
    }

    if let Some(effective) = provider_effective_output_cap(provider, model, request)
        && effective > limit
    {
        return Err(token_policy_error(effective, limit));
    }

    Ok(())
}

pub(super) fn prepare_chat_request_for_provider(
    max_tokens_per_request: Option<u32>,
    provider: &str,
    model: &str,
    mut core_request: ChatRequest,
    mut budget_request: ChatCompletionRequest,
) -> Result<(ChatRequest, ChatCompletionRequest), ProviderError> {
    core_request.model = model.to_string();
    apply_api_key_output_token_limit(max_tokens_per_request, provider, model, &mut core_request)?;
    budget_request.max_tokens = core_request.max_tokens;
    budget_request.max_completion_tokens = core_request.max_completion_tokens;
    Ok((core_request, budget_request))
}

fn provider_effective_output_cap(
    provider: &str,
    model: &str,
    request: &ChatRequest,
) -> Option<u32> {
    let provider = crate::core::pricing::normalize_pricing_provider(provider);
    match provider.as_str() {
        "openai" | "azure" | "azure_ai" | "openai_like" | "openrouter" | "xai" | "groq"
        | "deepseek" | "moonshot" | "minimax" | "zhipuai" | "xiaomi_mimo" | "amazon_nova"
        | "baseten" | "huggingface" => request.max_completion_tokens.or(request.max_tokens),
        "anthropic" => Some(request.max_tokens.unwrap_or(4096)),
        "bedrock" => bedrock_effective_output_cap(model, request),
        "cohere" | "replicate" => request.max_tokens.or(request.max_completion_tokens),
        _ => request.max_tokens,
    }
}

fn bedrock_effective_output_cap(model: &str, request: &ChatRequest) -> Option<u32> {
    use crate::core::providers::bedrock::BedrockApiType;

    let Ok(config) = crate::core::providers::bedrock::get_model_config_for_model_id(model) else {
        return request.max_tokens;
    };

    match config.api_type {
        BedrockApiType::Converse | BedrockApiType::ConverseStream => {
            request.max_completion_tokens.or(request.max_tokens)
        }
        BedrockApiType::Invoke | BedrockApiType::InvokeStream => request.max_tokens,
    }
}

fn token_policy_error(requested: u32, limit: u32) -> ProviderError {
    ProviderError::authentication(
        "api_key",
        format!("requested token limit {requested} exceeds API key max_tokens_per_request {limit}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_output_token_limit_uses_largest_supplied_cap() {
        assert_eq!(requested_output_token_limit(Some(100), Some(10)), Some(100));
        assert_eq!(requested_output_token_limit(None, Some(10)), Some(10));
    }

    #[test]
    fn rejects_bypass_when_legacy_max_tokens_exceeds_limit() {
        let mut request = ChatRequest {
            max_tokens: Some(100),
            max_completion_tokens: Some(10),
            ..Default::default()
        };

        assert!(
            apply_api_key_output_token_limit(Some(20), "anthropic", "claude-3-haiku", &mut request)
                .is_err()
        );
    }

    #[test]
    fn fills_provider_effective_cap_when_only_max_completion_tokens_is_set() {
        let mut request = ChatRequest {
            max_completion_tokens: Some(10),
            ..Default::default()
        };

        apply_api_key_output_token_limit(Some(20), "anthropic", "claude-3-haiku", &mut request)
            .expect("max_completion_tokens should cap max_tokens-only providers");

        assert_eq!(request.max_tokens, Some(10));
    }

    #[test]
    fn caps_provider_default_when_request_omits_token_limit() {
        let mut request = ChatRequest::default();

        apply_api_key_output_token_limit(Some(20), "anthropic", "claude-3-haiku", &mut request)
            .expect("missing token cap should be filled from key limit");

        assert_eq!(request.max_tokens, Some(20));
    }
}
