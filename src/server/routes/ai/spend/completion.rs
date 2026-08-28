use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::{
    ChatMessage, ContentPart, Function, FunctionCall, MessageContent, ResponseFormat, Tool,
};
use crate::core::pricing_service::PricingService;
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::ai::counter::token_counter::TokenCounter;
use crate::utils::ai::counter::types::TokenEstimate;
use crate::utils::error::gateway_error::Result as GatewayResult;

const IMAGE_PROMPT_BASE_TOKENS: u32 = 85;
pub(in crate::server::routes::ai) const IMAGE_HIGH_DETAIL_PROMPT_TOKENS: u32 = 1_105;
const AUDIO_PROMPT_BASE_TOKENS: u32 = 100;
const DOCUMENT_PROMPT_BASE_TOKENS: u32 = 1_000;
const TOOL_RESULT_BASE_TOKENS: u32 = 50;
const TOOL_USE_BASE_TOKENS: u32 = 100;

#[derive(Clone, Copy)]
pub(in crate::server::routes::ai) struct ChatCompletionBudgetRequest<'a> {
    messages: &'a [ChatMessage],
    tools: Option<&'a [Tool]>,
    functions: Option<&'a [Function]>,
    function_call: Option<&'a FunctionCall>,
    response_format: Option<&'a ResponseFormat>,
    max_tokens: Option<u32>,
    max_completion_tokens: Option<u32>,
    n: Option<u32>,
}

impl<'a> ChatCompletionBudgetRequest<'a> {
    pub(in crate::server::routes::ai) fn with_output_limits(
        mut self,
        max_tokens: Option<u32>,
        max_completion_tokens: Option<u32>,
    ) -> Self {
        self.max_tokens = max_tokens;
        self.max_completion_tokens = max_completion_tokens;
        self
    }
}

impl<'a> From<&'a ChatCompletionRequest> for ChatCompletionBudgetRequest<'a> {
    fn from(request: &'a ChatCompletionRequest) -> Self {
        Self {
            messages: &request.messages,
            tools: request.tools.as_deref(),
            functions: request.functions.as_deref(),
            function_call: request.function_call.as_ref(),
            response_format: request.response_format.as_ref(),
            max_tokens: request.max_tokens,
            max_completion_tokens: request.max_completion_tokens,
            n: request.n,
        }
    }
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn reserve_completion_budget(
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_completion_budget_with_pricing(
        super::default_spend_pricing_service(),
        budget_limits,
        provider,
        model,
        estimated_prompt_tokens,
        max_output_tokens,
    )
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn reserve_completion_budget_with_pricing(
    pricing_service: &PricingService,
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_completion_budget_with_policy(
        pricing_service,
        &GatewayPricingConfig::default(),
        budget_limits,
        provider,
        model,
        estimated_prompt_tokens,
        max_output_tokens,
    )
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn reserve_completion_budget_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_completion_budget_with_split_pricing(
        pricing_service,
        pricing_config,
        budget_limits,
        provider,
        model,
        provider,
        model,
        estimated_prompt_tokens,
        max_output_tokens,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::server::routes::ai) fn reserve_completion_budget_with_split_pricing(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let estimate = match pricing_service.estimate_loaded_completion_cost_for_provider(
        pricing_provider,
        pricing_model,
        estimated_prompt_tokens,
        max_output_tokens,
    ) {
        Ok(estimate) => estimate,
        Err(error) => {
            tracing::error!(
                "cost estimation failed for pricing provider '{pricing_provider}' model \
                 '{pricing_model}' budget provider '{budget_provider}' model '{budget_model}': {error}; \
                 applying unpriced model policy"
            );
            return super::unpriced::reserve_unpriced_completion_budget(
                pricing_config,
                budget_limits,
                budget_provider,
                budget_model,
                estimated_prompt_tokens,
                max_output_tokens,
                error,
            );
        }
    };

    if estimate.max_cost <= 0.0 {
        super::ensure_budget_available(budget_limits, budget_provider, budget_model)?;
        return Ok(None);
    }

    budget_limits
        .reserve_spend(budget_provider, budget_model, estimate.max_cost)
        .map(Some)
        .map_err(|error| {
            super::reservation_error_to_provider_error(error, budget_provider, budget_model)
        })
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn reserve_chat_completion_budget(
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_chat_completion_budget_with_pricing(
        super::default_spend_pricing_service(),
        budget_limits,
        provider,
        model,
        request,
    )
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn reserve_chat_completion_budget_with_pricing(
    pricing_service: &PricingService,
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_chat_completion_budget_with_policy(
        pricing_service,
        &GatewayPricingConfig::default(),
        budget_limits,
        provider,
        model,
        request,
    )
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn reserve_chat_completion_budget_with_policy(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_chat_completion_budget_with_split_pricing(
        pricing_service,
        pricing_config,
        budget_limits,
        provider,
        model,
        provider,
        model,
        request,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::server::routes::ai) fn reserve_chat_completion_budget_with_split_pricing<'a>(
    pricing_service: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    pricing_provider: &str,
    pricing_model: &str,
    request: impl Into<ChatCompletionBudgetRequest<'a>>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let request = request.into();
    let token_model = super::token_count_model_id(pricing_provider, pricing_model);
    let prompt_tokens = try_estimate_chat_prompt_tokens(
        &token_model,
        request.messages,
        request.tools,
        request.functions,
        request.function_call,
        request.response_format,
    )
    .map_err(|error| {
        super::token_count_error(
            budget_provider,
            budget_model,
            pricing_provider,
            pricing_model,
            error,
        )
    })?;
    let max_output_tokens = reservation_output_tokens(
        pricing_service,
        pricing_provider,
        pricing_model,
        prompt_tokens,
        provider_effective_max_output_tokens_for_budget(budget_provider, budget_model, request),
        request.n.unwrap_or(1),
    )
    .map_err(|error| {
        super::token_count_error(
            budget_provider,
            budget_model,
            pricing_provider,
            pricing_model,
            error,
        )
    })?;
    reserve_completion_budget_with_split_pricing(
        pricing_service,
        pricing_config,
        budget_limits,
        budget_provider,
        budget_model,
        pricing_provider,
        pricing_model,
        prompt_tokens,
        max_output_tokens,
    )
}

pub(in crate::server::routes::ai) fn estimate_chat_prompt_tokens(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[Tool]>,
    functions: Option<&[Function]>,
    function_call: Option<&FunctionCall>,
    response_format: Option<&ResponseFormat>,
) -> u32 {
    try_estimate_chat_prompt_tokens(
        model,
        messages,
        tools,
        functions,
        function_call,
        response_format,
    )
    .unwrap_or_else(|error| {
        tracing::error!(
            model = %model,
            error = %error,
            outcome = "conservative_max",
            "token counting failed outside budget reservation"
        );
        u32::MAX
    })
}

fn try_estimate_chat_prompt_tokens(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[Tool]>,
    functions: Option<&[Function]>,
    function_call: Option<&FunctionCall>,
    response_format: Option<&ResponseFormat>,
) -> GatewayResult<u32> {
    let counter = TokenCounter::new();
    let message_estimate = counter.count_chat_tokens(model, messages)?;
    observe_approximate_estimate(model, "messages", &message_estimate);
    let message_tokens = message_estimate.input_tokens;
    let multimodal_tokens = conservative_multimodal_prompt_extra(messages);

    let tool_tokens = if let Some(tools) = tools {
        let Ok(tool_json) = serde_json::to_string(tools) else {
            return Ok(u32::MAX);
        };
        let estimate = counter.count_completion_tokens(model, &tool_json)?;
        observe_approximate_estimate(model, "tools", &estimate);
        estimate.input_tokens
    } else {
        0
    };

    let function_tokens = serialized_prompt_tokens(
        &counter,
        model,
        functions,
        "legacy function token estimation failed",
        |functions| functions.len().saturating_mul(256),
    )?;
    let function_call_tokens = serialized_prompt_tokens(
        &counter,
        model,
        function_call,
        "legacy function_call token estimation failed",
        |_| 64,
    )?;
    let response_format_tokens = serialized_prompt_tokens(
        &counter,
        model,
        response_format,
        "response_format token estimation failed",
        |_| 128,
    )?;

    Ok(message_tokens
        .saturating_add(multimodal_tokens)
        .saturating_add(tool_tokens)
        .saturating_add(function_tokens)
        .saturating_add(function_call_tokens)
        .saturating_add(response_format_tokens))
}

fn observe_approximate_estimate(model: &str, input: &str, estimate: &TokenEstimate) {
    if estimate.is_approximate {
        tracing::warn!(
            model = %model,
            input,
            approximate = true,
            confidence = estimate.confidence,
            "approximate token count used"
        );
    }
}

fn reservation_output_tokens(
    pricing_service: &PricingService,
    provider: &str,
    model: &str,
    prompt_tokens: u32,
    requested_max_output_tokens: Option<u32>,
    choice_count: u32,
) -> GatewayResult<Option<u32>> {
    let counter = TokenCounter::new();
    let choice_count = choice_count.max(1);
    let output_tokens = if let Some(requested) = requested_max_output_tokens {
        Some(requested)
    } else {
        match catalog_max_output_tokens_with_pricing(pricing_service, provider, model) {
            Some(tokens) => Some(tokens),
            None => Some(counter.estimate_output_tokens(
                None,
                prompt_tokens,
                &super::token_count_model_id(provider, model),
            )?),
        }
    };

    Ok(output_tokens.map(|tokens| tokens.saturating_mul(choice_count)))
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn catalog_max_output_tokens(
    provider: &str,
    model: &str,
) -> Option<u32> {
    catalog_max_output_tokens_with_pricing(super::default_spend_pricing_service(), provider, model)
}

fn catalog_max_output_tokens_with_pricing(
    pricing_service: &PricingService,
    provider: &str,
    model: &str,
) -> Option<u32> {
    pricing_service.max_output_tokens_for_provider(provider, model)
}

#[cfg(test)]
pub(in crate::server::routes::ai) fn provider_effective_max_output_tokens(
    provider: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Option<u32> {
    provider_effective_max_output_tokens_for_budget(provider, model, request.into())
}

fn provider_effective_max_output_tokens_for_budget(
    provider: &str,
    model: &str,
    request: ChatCompletionBudgetRequest<'_>,
) -> Option<u32> {
    let provider = crate::core::pricing::normalize_pricing_provider(provider);
    match provider.as_str() {
        "openai" | "azure" | "azure_ai" | "openai_like" | "openrouter" | "xai" | "groq"
        | "deepseek" | "moonshot" | "minimax" | "zhipuai" | "xiaomi_mimo" | "amazon_nova"
        | "baseten" | "huggingface" | "zai" | "together_ai" | "fireworks_ai" | "aiml" => {
            request.max_completion_tokens.or(request.max_tokens)
        }
        "anthropic" => Some(request.max_tokens.unwrap_or(4096)),
        "bedrock" => bedrock_effective_max_output_tokens(model, request),
        "cohere" | "replicate" => request.max_tokens.or(request.max_completion_tokens),
        _ => request.max_tokens,
    }
}

fn bedrock_effective_max_output_tokens(
    model: &str,
    request: ChatCompletionBudgetRequest<'_>,
) -> Option<u32> {
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

fn conservative_multimodal_prompt_extra(messages: &[ChatMessage]) -> u32 {
    messages
        .iter()
        .filter_map(|message| message.content.as_ref())
        .flat_map(|content| match content {
            MessageContent::Text(_) => [].as_slice(),
            MessageContent::Parts(parts) => parts.as_slice(),
        })
        .fold(0u32, |total, part| {
            total.saturating_add(conservative_content_part_extra(part))
        })
}

fn conservative_content_part_extra(part: &ContentPart) -> u32 {
    match part {
        ContentPart::ImageUrl { image_url } => {
            image_prompt_floor(image_url.detail.as_deref()).saturating_sub(IMAGE_PROMPT_BASE_TOKENS)
        }
        ContentPart::Image {
            source,
            detail,
            image_url,
        } => {
            let detail = detail
                .as_deref()
                .or_else(|| image_url.as_ref().and_then(|url| url.detail.as_deref()));
            image_prompt_floor(detail)
                .max(encoded_media_tokens(&source.data))
                .saturating_sub(IMAGE_PROMPT_BASE_TOKENS)
        }
        ContentPart::Audio { audio } => encoded_media_tokens(&audio.data)
            .max(AUDIO_PROMPT_BASE_TOKENS)
            .saturating_sub(AUDIO_PROMPT_BASE_TOKENS),
        ContentPart::Document { source, .. } => encoded_media_tokens(&source.data)
            .max(DOCUMENT_PROMPT_BASE_TOKENS)
            .saturating_sub(DOCUMENT_PROMPT_BASE_TOKENS),
        ContentPart::ToolResult { .. } => {
            serialized_content_part_tokens(part).saturating_sub(TOOL_RESULT_BASE_TOKENS)
        }
        ContentPart::ToolUse { .. } => {
            serialized_content_part_tokens(part).saturating_sub(TOOL_USE_BASE_TOKENS)
        }
        ContentPart::Text { .. } => 0,
    }
}

fn serialized_content_part_tokens(part: &ContentPart) -> u32 {
    let Ok(json) = serde_json::to_string(part) else {
        return u32::MAX;
    };
    u32::try_from(json.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
}

fn image_prompt_floor(detail: Option<&str>) -> u32 {
    if detail.is_some_and(|detail| detail.eq_ignore_ascii_case("low")) {
        IMAGE_PROMPT_BASE_TOKENS
    } else {
        IMAGE_HIGH_DETAIL_PROMPT_TOKENS
    }
}

fn encoded_media_tokens(data: &str) -> u32 {
    u32::try_from(data.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
}

fn serialized_prompt_tokens<T, F>(
    counter: &TokenCounter,
    model: &str,
    value: Option<&T>,
    warn_message: &str,
    fallback_units: F,
) -> GatewayResult<u32>
where
    T: serde::Serialize + ?Sized,
    F: FnOnce(&T) -> usize,
{
    let Some(value) = value else {
        return Ok(0);
    };
    let Ok(json) = serde_json::to_string(value) else {
        return Ok(u32::try_from(fallback_units(value)).unwrap_or(u32::MAX));
    };

    let estimate = counter.count_completion_tokens(model, &json)?;
    observe_approximate_estimate(model, warn_message, &estimate);
    Ok(estimate.input_tokens)
}

#[cfg(test)]
mod budget_request_tests {
    use super::*;

    #[test]
    fn explicit_unknown_openai_tokenizer_fails_before_budget_estimation() {
        let pricing = PricingService::new(None);
        let budget = UnifiedBudgetLimits::new();
        let messages = vec![ChatMessage {
            role: crate::core::models::openai::MessageRole::User,
            content: Some(MessageContent::Text("hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];
        let request = ChatCompletionBudgetRequest {
            messages: &messages,
            tools: None,
            functions: None,
            function_call: None,
            response_format: None,
            max_tokens: Some(10),
            max_completion_tokens: None,
            n: None,
        };

        let error = match reserve_chat_completion_budget_with_split_pricing(
            &pricing,
            &GatewayPricingConfig::default(),
            &budget,
            "openai",
            "gpt-future-unknown",
            "openai",
            "gpt-future-unknown",
            request,
        ) {
            Err(error) => error,
            Ok(_) => panic!("explicit OpenAI tokenizer failure must stop budget estimation"),
        };

        let message = error.to_string();
        assert!(matches!(
            error,
            ProviderError::InvalidRequest {
                provider: "token_count",
                ..
            }
        ));
        assert!(message.contains("tokenizer resolution failed"));
        assert!(message.contains("gpt-future-unknown"));
    }

    #[test]
    fn budget_request_view_borrows_large_request_parts() {
        let request = ChatCompletionRequest {
            messages: vec![ChatMessage {
                role: crate::core::models::openai::MessageRole::User,
                content: Some(MessageContent::Text("x".repeat(64 * 1024))),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: crate::core::models::openai::Function {
                    name: "lookup".to_string(),
                    description: None,
                    parameters: Some(serde_json::json!({"type": "object"})),
                },
            }]),
            max_tokens: Some(1024),
            ..Default::default()
        };

        let view =
            ChatCompletionBudgetRequest::from(&request).with_output_limits(Some(128), Some(64));

        assert!(std::ptr::eq(
            view.messages.as_ptr(),
            request.messages.as_ptr()
        ));
        assert!(std::ptr::eq(
            view.tools.expect("tools").as_ptr(),
            request.tools.as_ref().expect("tools").as_ptr()
        ));
        assert_eq!(view.max_tokens, Some(128));
        assert_eq!(view.max_completion_tokens, Some(64));
    }
}
