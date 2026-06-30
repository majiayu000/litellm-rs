//! Spend and usage recording for completed requests.
//!
//! Wires the otherwise-dead budget and per-key usage tracking into the request
//! path: once a completion succeeds and its token usage is known, the served
//! provider/model budget spend and the calling key's usage are recorded.

mod key_budget;
mod pricing;

use uuid::Uuid;

use crate::core::budget::{
    BudgetReservation, BudgetReservationError, UnifiedBudgetLimits, UnifiedBudgetReservation,
};
use crate::core::keys::KeyManager;
use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::{
    ChatMessage, ContentPart, Function, FunctionCall, MessageContent, ResponseFormat, Tool,
};
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::Usage;
use crate::utils::ai::counter::token_counter::TokenCounter;
#[cfg(test)]
use std::sync::LazyLock;

pub(in crate::server::routes::ai) use key_budget::{
    reserve_api_key_budget, reserve_api_key_budget_for_reservation,
    settle_api_key_budget_reservation,
};
pub(super) use pricing::{
    pricing_identity_for_provider, record_pricing_usage_spend_with_reservation_with_pricing,
    reserve_embedding_budget_with_pricing, reserve_pricing_usage_budget_with_pricing,
};

const IMAGE_PROMPT_BASE_TOKENS: u32 = 85;
const IMAGE_HIGH_DETAIL_PROMPT_TOKENS: u32 = 1_105;
const AUDIO_PROMPT_BASE_TOKENS: u32 = 100;
const DOCUMENT_PROMPT_BASE_TOKENS: u32 = 1_000;
const TOOL_RESULT_BASE_TOKENS: u32 = 50;
const TOOL_USE_BASE_TOKENS: u32 = 100;

/// Reject a request before it reaches the upstream provider when the served
/// provider or model budget is already exhausted.
///
/// No-ops when budgets are disabled or unconfigured (the availability checks
/// return true). Returns a non-retryable `QuotaExceeded` error (HTTP 402) so
/// the router does not pointlessly retry an over-budget request.
pub(super) fn ensure_budget_available(
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
) -> Result<(), ProviderError> {
    if !budget_limits.is_provider_available(provider) {
        return Err(ProviderError::quota_exceeded(
            "budget",
            format!("provider '{provider}' budget exceeded"),
        ));
    }
    if !budget_limits.is_model_available(model) {
        return Err(ProviderError::quota_exceeded(
            "budget",
            format!("model '{model}' budget exceeded"),
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn reserve_completion_budget(
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_completion_budget_with_pricing(
        default_spend_pricing_service(),
        budget_limits,
        provider,
        model,
        estimated_prompt_tokens,
        max_output_tokens,
    )
}

pub(super) fn reserve_completion_budget_with_pricing(
    pricing_service: &PricingService,
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let estimate = match pricing_service.estimate_loaded_completion_cost_for_provider(
        provider,
        model,
        estimated_prompt_tokens,
        max_output_tokens,
    ) {
        Ok(estimate) => estimate,
        Err(e) => {
            tracing::error!(
                "cost estimation failed for '{provider}'/'{model}': {e}; \
                 checking exhausted status without reservation"
            );
            if pricing_required_for_budget(budget_limits, provider, model) {
                return Err(ProviderError::invalid_request(
                    "pricing",
                    format!(
                        "pricing is required for budget reservation for '{provider}'/'{model}': {e}"
                    ),
                ));
            }
            ensure_budget_available(budget_limits, provider, model)?;
            return Ok(None);
        }
    };

    if estimate.max_cost <= 0.0 {
        ensure_budget_available(budget_limits, provider, model)?;
        return Ok(None);
    }

    budget_limits
        .reserve_spend(provider, model, estimate.max_cost)
        .map(Some)
        .map_err(|error| reservation_error_to_provider_error(error, provider, model))
}

#[cfg(test)]
pub(super) fn reserve_chat_completion_budget(
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    reserve_chat_completion_budget_with_pricing(
        default_spend_pricing_service(),
        budget_limits,
        provider,
        model,
        request,
    )
}

pub(super) fn reserve_chat_completion_budget_with_pricing(
    pricing_service: &PricingService,
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let prompt_tokens = estimate_chat_prompt_tokens(
        model,
        &request.messages,
        request.tools.as_deref(),
        request.functions.as_deref(),
        request.function_call.as_ref(),
        request.response_format.as_ref(),
    );
    reserve_completion_budget_with_pricing(
        pricing_service,
        budget_limits,
        provider,
        model,
        prompt_tokens,
        reservation_output_tokens(
            pricing_service,
            provider,
            model,
            prompt_tokens,
            provider_effective_max_output_tokens(provider, model, request),
            request.n.unwrap_or(1),
        ),
    )
}

pub(super) fn estimate_chat_prompt_tokens(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[Tool]>,
    functions: Option<&[Function]>,
    function_call: Option<&FunctionCall>,
    response_format: Option<&ResponseFormat>,
) -> u32 {
    let counter = TokenCounter::new();
    let message_tokens = match counter.count_chat_tokens(model, messages) {
        Ok(estimate) => estimate.input_tokens,
        Err(error) => {
            tracing::warn!(
                "token estimation failed for model '{model}': {error}; using fallback estimate"
            );
            fallback_message_tokens(messages)
        }
    };
    let multimodal_tokens = conservative_multimodal_prompt_extra(messages);

    let tool_tokens = tools.map_or(0, |tools| {
        let Ok(tool_json) = serde_json::to_string(tools) else {
            return u32::try_from(tools.len().saturating_mul(256)).unwrap_or(u32::MAX);
        };
        counter
            .count_completion_tokens(model, &tool_json)
            .map(|estimate| estimate.input_tokens)
            .unwrap_or_else(|error| {
                tracing::warn!(
                    "tool token estimation failed for model '{model}': {error}; \
                     using fallback estimate"
                );
                u32::try_from(tool_json.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
            })
    });

    let function_tokens = serialized_prompt_tokens(
        &counter,
        model,
        functions,
        "legacy function token estimation failed",
        |functions| functions.len().saturating_mul(256),
    );
    let function_call_tokens = serialized_prompt_tokens(
        &counter,
        model,
        function_call,
        "legacy function_call token estimation failed",
        |_| 64,
    );
    let response_format_tokens = serialized_prompt_tokens(
        &counter,
        model,
        response_format,
        "response_format token estimation failed",
        |_| 128,
    );

    message_tokens
        .saturating_add(multimodal_tokens)
        .saturating_add(tool_tokens)
        .saturating_add(function_tokens)
        .saturating_add(function_call_tokens)
        .saturating_add(response_format_tokens)
}

fn reservation_output_tokens(
    pricing_service: &PricingService,
    provider: &str,
    model: &str,
    prompt_tokens: u32,
    requested_max_output_tokens: Option<u32>,
    choice_count: u32,
) -> Option<u32> {
    let counter = TokenCounter::new();
    let choice_count = choice_count.max(1);
    let output_tokens = if let Some(requested) = requested_max_output_tokens {
        Some(requested)
    } else {
        catalog_max_output_tokens_with_pricing(pricing_service, provider, model).or_else(|| {
            counter
                .estimate_output_tokens(None, prompt_tokens, model)
                .ok()
        })
    };

    output_tokens.map(|tokens| tokens.saturating_mul(choice_count))
}

#[cfg(test)]
fn catalog_max_output_tokens(provider: &str, model: &str) -> Option<u32> {
    catalog_max_output_tokens_with_pricing(default_spend_pricing_service(), provider, model)
}

fn catalog_max_output_tokens_with_pricing(
    pricing_service: &PricingService,
    provider: &str,
    model: &str,
) -> Option<u32> {
    pricing_service.max_output_tokens_for_provider(provider, model)
}

fn provider_effective_max_output_tokens(
    provider: &str,
    model: &str,
    request: &ChatCompletionRequest,
) -> Option<u32> {
    let provider = crate::core::pricing::normalize_pricing_provider(provider);
    match provider.as_str() {
        "openai" | "azure" | "azure_ai" | "openai_like" | "openrouter" | "xai" | "groq"
        | "deepseek" | "moonshot" | "minimax" | "zhipuai" | "xiaomi_mimo" | "amazon_nova"
        | "baseten" | "huggingface" => request.max_completion_tokens.or(request.max_tokens),
        "anthropic" => Some(request.max_tokens.unwrap_or(4096)),
        "bedrock" => bedrock_effective_max_output_tokens(model, request),
        "cohere" | "replicate" => request.max_tokens.or(request.max_completion_tokens),
        _ => request.max_tokens,
    }
}

fn bedrock_effective_max_output_tokens(
    model: &str,
    request: &ChatCompletionRequest,
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
) -> u32
where
    T: serde::Serialize + ?Sized,
    F: FnOnce(&T) -> usize,
{
    let Some(value) = value else {
        return 0;
    };
    let Ok(json) = serde_json::to_string(value) else {
        return u32::try_from(fallback_units(value)).unwrap_or(u32::MAX);
    };

    counter
        .count_completion_tokens(model, &json)
        .map(|estimate| estimate.input_tokens)
        .unwrap_or_else(|error| {
            tracing::warn!("{warn_message} for model '{model}': {error}; using fallback estimate");
            u32::try_from(json.chars().count().div_ceil(4)).unwrap_or(u32::MAX)
        })
}

fn fallback_message_tokens(messages: &[ChatMessage]) -> u32 {
    let chars = messages
        .iter()
        .filter_map(|message| message.content.as_ref())
        .map(|content| match content {
            MessageContent::Text(text) => text.chars().count(),
            MessageContent::Parts(parts) => serde_json::to_string(parts)
                .map(|text| text.chars().count())
                .unwrap_or_default(),
        })
        .sum::<usize>();
    let overhead = messages.len().saturating_mul(4).saturating_add(8);
    u32::try_from(chars.div_ceil(4).saturating_add(overhead)).unwrap_or(u32::MAX)
}

pub(in crate::server::routes::ai) fn reservation_error_to_provider_error(
    error: BudgetReservationError,
    provider: &str,
    model: &str,
) -> ProviderError {
    match error {
        BudgetReservationError::ProviderBudgetExceeded => ProviderError::quota_exceeded(
            "budget",
            format!("provider '{provider}' budget exceeded"),
        ),
        BudgetReservationError::ModelBudgetExceeded => {
            ProviderError::quota_exceeded("budget", format!("model '{model}' budget exceeded"))
        }
        BudgetReservationError::BudgetExceeded => ProviderError::quota_exceeded(
            "budget",
            format!("budget exceeded for provider '{provider}' model '{model}'"),
        ),
        BudgetReservationError::InvalidAmount(error) => ProviderError::invalid_request(
            "budget",
            format!("invalid budget reservation amount for '{provider}'/'{model}': {error}"),
        ),
        BudgetReservationError::ActualExceedsReservation => ProviderError::invalid_request(
            "budget",
            format!("actual spend exceeded reserved budget for '{provider}'/'{model}'"),
        ),
    }
}

/// Record provider/model budget spend and per-key usage for a completed request.
///
/// Best-effort and non-fatal: the completion already succeeded, so failures here
/// are logged at error level (never silently swallowed) but do not fail the
/// response. When the cost cannot be priced, token usage is still recorded but
/// budget spend is skipped rather than booked at $0 — under-counting a budget is
/// worse than leaving it unchanged with a loud error.
pub(super) struct UsageSpendSettlement<'a> {
    pub(super) budget_limits: &'a UnifiedBudgetLimits,
    pub(super) key_manager: &'a KeyManager,
    pub(super) api_key_id: Option<Uuid>,
    pub(super) provider: &'a str,
    pub(super) model: &'a str,
    pub(super) usage: Option<&'a Usage>,
    pub(super) budget_reservation: Option<UnifiedBudgetReservation>,
    pub(super) key_budget_reservation: Option<BudgetReservation>,
}

pub(super) fn usage_spend_settlement<'a>(
    core: (&'a UnifiedBudgetLimits, &'a KeyManager, Option<Uuid>),
    usage: (&'a str, &'a str, Option<&'a Usage>),
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) -> UsageSpendSettlement<'a> {
    let (budget_limits, key_manager, api_key_id) = core;
    let (provider, model, usage) = usage;
    UsageSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        usage,
        budget_reservation,
        key_budget_reservation,
    }
}

#[cfg(test)]
pub(super) async fn record_completion_spend_with_reservation(settlement: UsageSpendSettlement<'_>) {
    record_completion_spend_with_reservation_with_pricing(
        default_spend_pricing_service(),
        settlement,
    )
    .await;
}

pub(super) async fn record_completion_spend_with_reservation_with_pricing(
    pricing_service: &PricingService,
    settlement: UsageSpendSettlement<'_>,
) {
    let UsageSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        usage,
        budget_reservation,
        key_budget_reservation,
    } = settlement;

    let Some(usage) = usage else {
        record_reserved_spend_without_usage(
            key_manager,
            api_key_id,
            provider,
            model,
            budget_reservation,
            key_budget_reservation,
            "provider returned no usage for a successful completion",
        )
        .await;
        return;
    };

    let total_tokens = u64::from(usage.total_tokens);
    let usage_tokens = PricingUsage::from(usage);

    let cost = match pricing_service.calculate_loaded_usage_cost_for_provider(
        provider,
        model,
        &usage_tokens,
    ) {
        Ok(breakdown) => Some(breakdown.total_cost),
        Err(e) => {
            tracing::error!(
                "cost calculation failed for '{provider}'/'{model}': {e}; \
                 recording token usage without cost and skipping budget spend"
            );
            None
        }
    };

    if let Some(cost) = cost {
        if let Some(reservation) = budget_reservation {
            if let Err(error) = reservation.settle(cost) {
                tracing::error!(
                    "failed to settle reserved budget for '{provider}'/'{model}': {error:?}; \
                     spend not recorded because reservation settlement failed"
                );
            }
        } else {
            budget_limits.record_spend(provider, model, cost);
        }
        settle_api_key_budget_reservation(
            key_budget_reservation,
            cost,
            &format!("{provider}/{model}"),
        );
    }

    if let Some(key_id) = api_key_id {
        // Token counts are factual even when pricing is unavailable; record them
        // with the cost we have (0.0 only when pricing failed, already logged).
        if let Err(e) = key_manager
            .record_usage(key_id, total_tokens, cost.unwrap_or(0.0))
            .await
        {
            tracing::error!("failed to record usage for key {key_id}: {e}");
        }
    }
}

async fn record_reserved_spend_without_usage(
    key_manager: &KeyManager,
    api_key_id: Option<Uuid>,
    provider: &str,
    model: &str,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    context: &str,
) {
    let Some(reservation) = budget_reservation else {
        tracing::error!("{context} for provider '{provider}' model '{model}'; spend not recorded");
        return;
    };
    let reserved = reservation.reserved_amount();
    if let Err(error) = reservation.settle(reserved) {
        tracing::error!(
            "failed to settle reserved budget without usage for '{provider}'/'{model}': {error:?}"
        );
    }
    if let Some(key_id) = api_key_id
        && let Err(error) = key_manager.record_usage(key_id, 0, reserved).await
    {
        tracing::error!("failed to record reserved usage for key {key_id}: {error}");
    }
    settle_api_key_budget_reservation(key_budget_reservation, reserved, context);
}

#[cfg(test)]
pub(super) async fn record_stream_disconnect_spend_with_reservation(
    settlement: UsageSpendSettlement<'_>,
) {
    record_stream_disconnect_spend_with_reservation_with_pricing(
        default_spend_pricing_service(),
        settlement,
    )
    .await;
}

pub(super) async fn record_stream_disconnect_spend_with_reservation_with_pricing(
    pricing_service: &PricingService,
    settlement: UsageSpendSettlement<'_>,
) {
    let UsageSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        usage,
        budget_reservation,
        key_budget_reservation,
    } = settlement;

    if let Some(usage) = usage {
        record_completion_spend_with_reservation_with_pricing(
            pricing_service,
            usage_spend_settlement(
                (budget_limits, key_manager, api_key_id),
                (provider, model, Some(usage)),
                budget_reservation,
                key_budget_reservation,
            ),
        )
        .await;
        return;
    }

    record_reserved_spend_without_usage(
        key_manager,
        api_key_id,
        provider,
        model,
        budget_reservation,
        key_budget_reservation,
        "client disconnected before provider returned usage",
    )
    .await;
}

pub(super) struct StreamSpendSettlement<'a> {
    pub(super) budget_limits: &'a UnifiedBudgetLimits,
    pub(super) key_manager: &'a KeyManager,
    pub(super) api_key_id: Option<Uuid>,
    pub(super) provider: &'a str,
    pub(super) model: &'a str,
    pub(super) usage: Option<&'a Usage>,
    pub(super) saw_upstream_output: bool,
    pub(super) budget_reservation: Option<UnifiedBudgetReservation>,
    pub(super) key_budget_reservation: Option<BudgetReservation>,
}

#[cfg(test)]
pub(super) async fn record_finished_stream_spend_with_reservation(
    settlement: StreamSpendSettlement<'_>,
) {
    record_finished_stream_spend_with_reservation_with_pricing(
        default_spend_pricing_service(),
        settlement,
    )
    .await;
}

pub(super) async fn record_finished_stream_spend_with_reservation_with_pricing(
    pricing_service: &PricingService,
    settlement: StreamSpendSettlement<'_>,
) {
    let StreamSpendSettlement {
        budget_limits,
        key_manager,
        api_key_id,
        provider,
        model,
        usage,
        saw_upstream_output,
        budget_reservation,
        key_budget_reservation,
    } = settlement;

    if usage.is_some() || saw_upstream_output {
        record_stream_disconnect_spend_with_reservation_with_pricing(
            pricing_service,
            usage_spend_settlement(
                (budget_limits, key_manager, api_key_id),
                (provider, model, usage),
                budget_reservation,
                key_budget_reservation,
            ),
        )
        .await;
        return;
    }

    record_completion_spend_with_reservation_with_pricing(
        pricing_service,
        usage_spend_settlement(
            (budget_limits, key_manager, api_key_id),
            (provider, model, usage),
            budget_reservation,
            key_budget_reservation,
        ),
    )
    .await;
}

fn pricing_required_for_budget(
    budget_limits: &UnifiedBudgetLimits,
    provider: &str,
    model: &str,
) -> bool {
    let provider_limit_enabled = budget_limits.providers.is_enabled()
        && budget_limits
            .providers
            .list_provider_budgets()
            .into_iter()
            .any(|budget| budget.provider_name == provider && budget.enabled);
    let model_limit_enabled = budget_limits.models.is_enabled()
        && budget_limits
            .models
            .list_model_budgets()
            .into_iter()
            .any(|budget| budget.model_name == model && budget.enabled);

    provider_limit_enabled || model_limit_enabled
}

#[cfg(test)]
fn default_spend_pricing_service() -> &'static PricingService {
    static DEFAULT_SPEND_PRICING_SERVICE: LazyLock<PricingService> = LazyLock::new(|| {
        PricingService::with_embedded_default().unwrap_or_else(|error| {
            tracing::error!("failed to initialize embedded spend PricingService: {error}");
            PricingService::new(None)
        })
    });
    &DEFAULT_SPEND_PRICING_SERVICE
}

#[cfg(test)]
#[path = "spend_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "spend_provider_reservation_tests.rs"]
mod provider_reservation_tests;

#[cfg(test)]
#[path = "spend_provider_output_cap_tests.rs"]
mod provider_output_cap_tests;

#[cfg(test)]
#[path = "spend_stream_disconnect_tests.rs"]
mod stream_disconnect_tests;

#[cfg(test)]
#[path = "spend_no_usage_tests.rs"]
mod no_usage_tests;

#[cfg(test)]
#[path = "spend_runtime_pricing_tests.rs"]
mod runtime_pricing_tests;
