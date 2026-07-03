use bytes::Bytes;
use serde_json::Value;
use tracing::error;

use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{
    BudgetReservation, BudgetReservationError, UnifiedBudgetLimits, UnifiedBudgetReservation,
};
use crate::core::keys::KeyManager;
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::ProviderError;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::provider::GeminiRouteProvider;

pub(super) struct GeminiSpendState<'a> {
    pub(super) pricing: &'a PricingService,
    pub(super) pricing_config: &'a GatewayPricingConfig,
    pub(super) budget_limits: &'a UnifiedBudgetLimits,
    pub(super) key_manager: &'a KeyManager,
    pub(super) api_key_id: Option<uuid::Uuid>,
}

pub(super) async fn settle_gemini_stream_spend(
    spend_state: &GeminiSpendState<'_>,
    provider: &GeminiRouteProvider,
    usage: Option<PricingUsage>,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    saw_upstream_output: bool,
) {
    if let Some(usage) = usage {
        record_gemini_usage(
            spend_state,
            provider,
            usage,
            budget_reservation,
            key_budget_reservation,
        )
        .await;
    } else if saw_upstream_output {
        settle_gemini_reserved_spend_without_usage(
            spend_state,
            provider,
            budget_reservation,
            key_budget_reservation,
            "Gemini SDK stream ended without usageMetadata",
        )
        .await;
    }
}

pub(super) fn extract_gemini_sse_usage(bytes: &Bytes, buffer: &mut String) -> Option<PricingUsage> {
    buffer.push_str(&String::from_utf8_lossy(bytes));
    let mut usage = None;
    while let Some((event_end, separator_len)) = next_sse_event_boundary(buffer) {
        let event = buffer[..event_end].to_string();
        buffer.drain(..event_end + separator_len);
        if let Some(next_usage) = parse_gemini_sse_event_usage(&event) {
            usage = Some(next_usage);
        }
    }
    usage
}

fn next_sse_event_boundary(buffer: &str) -> Option<(usize, usize)> {
    match (buffer.find("\n\n"), buffer.find("\r\n\r\n")) {
        (Some(lf), Some(crlf)) if lf < crlf => Some((lf, 2)),
        (Some(_), Some(crlf)) => Some((crlf, 4)),
        (Some(lf), None) => Some((lf, 2)),
        (None, Some(crlf)) => Some((crlf, 4)),
        (None, None) => None,
    }
}

pub(super) async fn record_gemini_spend(
    spend_state: &GeminiSpendState<'_>,
    provider: &GeminiRouteProvider,
    body: &[u8],
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    settle_without_usage: bool,
) {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        settle_gemini_reserved_spend_without_usage(
            spend_state,
            provider,
            budget_reservation,
            key_budget_reservation,
            "Gemini SDK response was not valid JSON",
        )
        .await;
        return;
    };
    let Some(usage) = gemini_usage_metadata(&value) else {
        if settle_without_usage {
            settle_gemini_reserved_spend_without_usage(
                spend_state,
                provider,
                budget_reservation,
                key_budget_reservation,
                "Gemini SDK response had no usageMetadata",
            )
            .await;
        }
        return;
    };
    record_gemini_usage(
        spend_state,
        provider,
        usage,
        budget_reservation,
        key_budget_reservation,
    )
    .await;
}

pub(super) fn reserve_gemini_budget(
    state: &AppState,
    provider: &GeminiRouteProvider,
    request: &Value,
) -> Result<Option<UnifiedBudgetReservation>, GatewayError> {
    let usage = estimated_gemini_request_usage(request);
    let pricing_config = &state.config().gateway.pricing;
    let estimate = match state.pricing.estimate_loaded_completion_cost_for_provider(
        &provider.pricing_provider,
        &provider.model,
        usage.prompt_tokens,
        Some(usage.completion_tokens),
    ) {
        Ok(estimate) => estimate,
        Err(error) => {
            return super::super::spend::reserve_unpriced_usage_budget(
                pricing_config,
                &state.budget_limits,
                &provider.provider_name,
                &provider.model,
                &usage,
                error,
            )
            .map_err(GatewayError::Provider);
        }
    };
    if estimate.max_cost <= 0.0 {
        super::super::spend::ensure_budget_available(
            &state.budget_limits,
            &provider.provider_name,
            &provider.model,
        )?;
        return Ok(None);
    }
    state
        .budget_limits
        .reserve_spend(&provider.provider_name, &provider.model, estimate.max_cost)
        .map(Some)
        .map_err(|error| reservation_error_to_gateway_error(error, provider))
}

async fn record_gemini_usage(
    spend_state: &GeminiSpendState<'_>,
    provider: &GeminiRouteProvider,
    usage: PricingUsage,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) {
    let cost = match spend_state
        .pricing
        .calculate_loaded_usage_cost_for_provider(
            &provider.pricing_provider,
            &provider.model,
            &usage,
        ) {
        Ok(breakdown) => breakdown.total_cost,
        Err(error) => {
            super::super::spend::settle_unpriced_usage(
                spend_state.pricing_config,
                spend_state.budget_limits,
                spend_state.key_manager,
                spend_state.api_key_id,
                &provider.provider_name,
                &provider.model,
                &usage,
                budget_reservation,
                key_budget_reservation,
                &format!("Gemini SDK cost calculation failed: {error}"),
            )
            .await;
            return;
        }
    };

    if let Some(reservation) = budget_reservation {
        if let Err(error) = reservation.settle(cost) {
            error!(
                "failed to settle Gemini SDK budget for provider '{}' model '{}': {error:?}",
                provider.provider_name, provider.model
            );
        }
    } else {
        spend_state
            .budget_limits
            .record_spend(&provider.provider_name, &provider.model, cost);
    }
    super::super::spend::settle_api_key_budget_reservation(
        key_budget_reservation,
        cost,
        "Gemini SDK spend",
    );

    if let Some(key_id) = spend_state.api_key_id
        && let Err(error) = spend_state
            .key_manager
            .record_usage(key_id, u64::from(usage.total_tokens), cost)
            .await
    {
        error!("failed to record Gemini SDK usage for key {key_id}: {error}");
    }
}

async fn settle_gemini_reserved_spend_without_usage(
    spend_state: &GeminiSpendState<'_>,
    provider: &GeminiRouteProvider,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    context: &str,
) {
    let Some(reservation) = budget_reservation else {
        super::super::spend::settle_api_key_budget_reservation(
            key_budget_reservation,
            0.0,
            context,
        );
        error!(
            "{context} for provider '{}' model '{}'; spend not recorded",
            provider.provider_name, provider.model
        );
        return;
    };
    let reserved = reservation.reserved_amount();
    if let Err(error) = reservation.settle(reserved) {
        error!(
            "failed to settle reserved Gemini SDK budget for provider '{}' model '{}': {error:?}",
            provider.provider_name, provider.model
        );
    }
    super::super::spend::settle_api_key_budget_reservation(
        key_budget_reservation,
        reserved,
        context,
    );
    if let Some(key_id) = spend_state.api_key_id
        && let Err(error) = spend_state
            .key_manager
            .record_usage(key_id, 0, reserved)
            .await
    {
        error!("failed to record reserved Gemini SDK usage for key {key_id}: {error}");
    }
    error!(
        "{context} for provider '{}' model '{}'; charged reserved amount",
        provider.provider_name, provider.model
    );
}

fn reservation_error_to_gateway_error(
    error: BudgetReservationError,
    provider: &GeminiRouteProvider,
) -> GatewayError {
    let message = match error {
        BudgetReservationError::BudgetExceeded => "budget exceeded".to_string(),
        BudgetReservationError::ProviderBudgetExceeded => {
            format!("provider '{}' budget exceeded", provider.provider_name)
        }
        BudgetReservationError::ModelBudgetExceeded => {
            format!("model '{}' budget exceeded", provider.model)
        }
        BudgetReservationError::InvalidAmount(error) => format!("invalid budget amount: {error}"),
        BudgetReservationError::ActualExceedsReservation => format!(
            "actual spend exceeded reserved budget for '{}/{}'",
            provider.provider_name, provider.model
        ),
    };
    match error {
        BudgetReservationError::BudgetExceeded
        | BudgetReservationError::ProviderBudgetExceeded
        | BudgetReservationError::ModelBudgetExceeded => {
            GatewayError::from(ProviderError::quota_exceeded("budget", message))
        }
        BudgetReservationError::InvalidAmount(_)
        | BudgetReservationError::ActualExceedsReservation => {
            GatewayError::from(ProviderError::invalid_request("budget", message))
        }
    }
}

fn parse_gemini_sse_event_usage(event: &str) -> Option<PricingUsage> {
    for line in event.lines() {
        let Some(data) = line.trim_start().strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" || data.is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(data).ok()?;
        if let Some(usage) = gemini_usage_metadata(&value) {
            return Some(usage);
        }
    }
    None
}

fn estimated_gemini_request_usage(request: &Value) -> PricingUsage {
    let prompt_tokens = estimate_gemini_prompt_tokens(request);
    let completion_tokens = request
        .pointer("/generationConfig/maxOutputTokens")
        .and_then(Value::as_u64)
        .and_then(|tokens| u32::try_from(tokens).ok())
        .unwrap_or(0);
    PricingUsage::new(prompt_tokens, completion_tokens)
}

fn gemini_usage_metadata(value: &Value) -> Option<PricingUsage> {
    let metadata = value.get("usageMetadata")?;
    let prompt_tokens = u32_field(metadata, "promptTokenCount").unwrap_or(0);
    let completion_tokens = u32_field(metadata, "candidatesTokenCount").unwrap_or(0);
    let mut usage = PricingUsage::new(prompt_tokens, completion_tokens);
    usage.total_tokens = u32_field(metadata, "totalTokenCount")
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    usage.cached_tokens = u32_field(metadata, "cachedContentTokenCount");
    usage.reasoning_tokens = u32_field(metadata, "thoughtsTokenCount");
    Some(usage)
}

fn u32_field(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|tokens| u32::try_from(tokens).ok())
}

fn estimate_gemini_prompt_tokens(request: &Value) -> u32 {
    let mut chars = 0_usize;
    if let Some(contents) = request.get("contents") {
        collect_text_chars(contents, &mut chars);
    }
    if let Some(system_instruction) = request.get("systemInstruction") {
        collect_text_chars(system_instruction, &mut chars);
    }
    if chars == 0 {
        chars = serde_json::to_string(request)
            .map(|serialized| serialized.chars().count())
            .unwrap_or(0);
    }
    u32::try_from(chars.div_ceil(4).max(1)).unwrap_or(u32::MAX)
}

fn collect_text_chars(value: &Value, chars: &mut usize) {
    match value {
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                *chars = chars.saturating_add(text.chars().count());
            }
            for child in map.values() {
                collect_text_chars(child, chars);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_text_chars(item, chars);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget::{BudgetConfig, BudgetManager, BudgetScope};
    use crate::core::keys::{InMemoryKeyRepository, KeyManager};
    use crate::core::pricing_service::PricingService;
    use crate::server::routes::ai::gemini::provider::test_gemini_route_provider;

    #[test]
    fn extracts_usage_from_crlf_sse_event_boundaries() {
        let mut buffer = String::new();
        let usage = extract_gemini_sse_usage(
            &Bytes::from_static(
                b"event: message\r\n\
                  data: {\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":2,\"totalTokenCount\":3}}\r\n\r\n",
            ),
            &mut buffer,
        );
        assert!(usage.is_some(), "usage should be parsed");
        let usage = usage.unwrap_or_default();

        assert_eq!(usage.prompt_tokens, 1);
        assert_eq!(usage.completion_tokens, 2);
        assert_eq!(usage.total_tokens, 3);
        assert!(buffer.is_empty());
    }

    #[tokio::test]
    async fn record_gemini_spend_settles_api_key_budget_reservation() {
        let pricing = PricingService::with_embedded_default()
            .unwrap_or_else(|error| panic!("embedded pricing should load: {error}"));
        let budget_limits = UnifiedBudgetLimits::new();
        let key_manager = KeyManager::new(InMemoryKeyRepository::new());
        let budget_manager = BudgetManager::new();
        let scope = BudgetScope::ApiKey("gemini-key-budget".to_string());
        budget_manager
            .create_budget(scope.clone(), BudgetConfig::new("gemini key", 1.0))
            .await
            .unwrap_or_else(|error| panic!("API key budget should be created: {error}"));
        let key_budget_reservation = budget_manager
            .tracker()
            .reserve_spend(&scope, 0.5)
            .unwrap_or_else(|error| panic!("API key budget should reserve: {error:?}"));
        let provider = test_gemini_route_provider("openai", "openai", "gpt-4o");
        let pricing_config = GatewayPricingConfig::default();
        let spend_state = GeminiSpendState {
            pricing: &pricing,
            pricing_config: &pricing_config,
            budget_limits: &budget_limits,
            key_manager: &key_manager,
            api_key_id: None,
        };

        record_gemini_spend(
            &spend_state,
            &provider,
            br#"{"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5,"totalTokenCount":15}}"#,
            None,
            Some(key_budget_reservation),
            true,
        )
        .await;

        let spend = budget_manager.get_current_spend(&scope);
        assert!(spend > 0.0, "API key budget spend should be recorded");
        assert!(spend < 0.5, "reservation should settle to actual spend");
    }
}
