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
use crate::core::providers::shared::strict_direct_gemini_usage_metadata;
use crate::utils::error::gateway_error::GatewayError;

use super::provider::GeminiRouteProvider;

pub(super) struct GeminiSpendState<'a> {
    pub(super) pricing: &'a PricingService,
    pub(super) pricing_config: &'a GatewayPricingConfig,
    pub(super) budget_limits: &'a UnifiedBudgetLimits,
    pub(super) key_manager: &'a KeyManager,
    pub(super) api_key_id: Option<uuid::Uuid>,
}

#[derive(Debug, Default)]
pub(super) enum GeminiStreamUsage {
    #[default]
    Missing,
    Valid(PricingUsage),
    Invalid,
}

impl GeminiStreamUsage {
    pub(super) fn observe(&mut self, observation: Self) {
        match observation {
            Self::Missing => {}
            observation => *self = observation,
        }
    }

    pub(super) fn as_valid(&self) -> Option<&PricingUsage> {
        match self {
            Self::Valid(usage) => Some(usage),
            Self::Missing | Self::Invalid => None,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct GeminiSseObservation {
    pub(super) usage: GeminiStreamUsage,
    pub(super) saw_candidate_output: bool,
}

impl GeminiSseObservation {
    fn observe(&mut self, observation: Self) {
        self.usage.observe(observation.usage);
        self.saw_candidate_output |= observation.saw_candidate_output;
    }
}

pub(super) async fn settle_gemini_stream_spend(
    spend_state: &GeminiSpendState<'_>,
    provider: &GeminiRouteProvider,
    usage: GeminiStreamUsage,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    saw_upstream_output: bool,
) {
    match usage {
        GeminiStreamUsage::Valid(usage) => {
            record_gemini_usage(
                spend_state,
                provider,
                usage,
                budget_reservation,
                key_budget_reservation,
            )
            .await;
        }
        GeminiStreamUsage::Missing if saw_upstream_output => {
            settle_gemini_reserved_spend_without_usage(
                spend_state,
                provider,
                budget_reservation,
                key_budget_reservation,
                "Gemini SDK stream ended without usageMetadata",
            )
            .await;
        }
        GeminiStreamUsage::Invalid if saw_upstream_output => {
            settle_gemini_reserved_spend_without_usage(
                spend_state,
                provider,
                budget_reservation,
                key_budget_reservation,
                "Gemini SDK stream ended with invalid usageMetadata",
            )
            .await;
        }
        GeminiStreamUsage::Missing | GeminiStreamUsage::Invalid => {}
    }
}

#[cfg(test)]
pub(super) fn extract_gemini_sse_usage(bytes: &Bytes, buffer: &mut String) -> GeminiStreamUsage {
    extract_gemini_sse_observation(bytes, buffer).usage
}

pub(super) fn extract_gemini_sse_observation(
    bytes: &Bytes,
    buffer: &mut String,
) -> GeminiSseObservation {
    buffer.push_str(&String::from_utf8_lossy(bytes));
    let mut observation = GeminiSseObservation::default();
    while let Some((event_end, separator_len)) = next_sse_event_boundary(buffer) {
        let event = buffer[..event_end].to_string();
        buffer.drain(..event_end + separator_len);
        observation.observe(parse_gemini_sse_event(&event));
    }
    observation
}

#[cfg(test)]
pub(super) fn finish_gemini_sse_usage(buffer: &mut String) -> GeminiStreamUsage {
    finish_gemini_sse_observation(buffer).usage
}

pub(super) fn finish_gemini_sse_observation(buffer: &mut String) -> GeminiSseObservation {
    let event = std::mem::take(buffer);
    if event.trim().is_empty() {
        GeminiSseObservation::default()
    } else {
        parse_gemini_sse_event(&event)
    }
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
    pricing: &PricingService,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    provider: &GeminiRouteProvider,
    request: &Value,
) -> Result<Option<UnifiedBudgetReservation>, GatewayError> {
    let usage = estimated_gemini_request_usage(request);
    let estimate = match pricing.estimate_loaded_completion_cost_for_provider(
        &provider.pricing_provider,
        &provider.model,
        usage.prompt_tokens,
        Some(usage.completion_tokens),
    ) {
        Ok(estimate) => estimate,
        Err(error) => {
            return super::super::spend::reserve_unpriced_usage_budget(
                pricing_config,
                budget_limits,
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
            budget_limits,
            &provider.provider_name,
            &provider.model,
        )?;
        return Ok(None);
    }
    budget_limits
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
    super::super::spend::record_reserved_spend_without_usage(
        spend_state.key_manager,
        spend_state.api_key_id,
        &provider.provider_name,
        &provider.model,
        budget_reservation,
        key_budget_reservation,
        context,
    )
    .await;
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

fn parse_gemini_sse_event(event: &str) -> GeminiSseObservation {
    let mut observation = GeminiSseObservation::default();
    for line in event.lines() {
        let Some(data) = line.trim_start().strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" || data.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            observation.usage = GeminiStreamUsage::Invalid;
            return observation;
        };
        observation.saw_candidate_output |= value
            .get("candidates")
            .and_then(Value::as_array)
            .is_some_and(|candidates| !candidates.is_empty());
        if value.get("usageMetadata").is_none() {
            continue;
        }
        let Some(next_usage) = gemini_usage_metadata(&value) else {
            observation.usage = GeminiStreamUsage::Invalid;
            return observation;
        };
        observation.usage = GeminiStreamUsage::Valid(next_usage);
    }
    observation
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
    let usage = strict_direct_gemini_usage_metadata(metadata)?;
    Some(PricingUsage::from(&usage))
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
    use crate::core::budget::{
        BudgetConfig, BudgetManager, BudgetScope, ModelLimitConfig, ProviderLimitConfig,
        ResetPeriod,
    };
    use crate::core::keys::{CreateKeyConfig, InMemoryKeyRepository, KeyManager};
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
        let GeminiStreamUsage::Valid(usage) = usage else {
            panic!("usage should be parsed");
        };

        assert_eq!(usage.prompt_tokens, 1);
        assert_eq!(usage.completion_tokens, 2);
        assert_eq!(usage.total_tokens, 3);
        assert!(buffer.is_empty());
    }

    #[test]
    fn sse_observation_ignores_heartbeat_as_candidate_output() {
        let mut buffer = String::new();
        let heartbeat =
            extract_gemini_sse_observation(&Bytes::from_static(b": keepalive\n\n"), &mut buffer);
        assert!(!heartbeat.saw_candidate_output);
        assert!(matches!(heartbeat.usage, GeminiStreamUsage::Missing));

        let output = extract_gemini_sse_observation(
            &Bytes::from_static(b"data: {\"candidates\":[{\"content\":{}}]}\n\n"),
            &mut buffer,
        );
        assert!(output.saw_candidate_output);
    }

    #[test]
    fn native_sse_usage_distinguishes_missing_invalid_and_valid_updates() {
        let mut buffer = String::new();
        let mut usage = GeminiStreamUsage::Missing;
        usage.observe(extract_gemini_sse_usage(
            &Bytes::from_static(
                b"data: {\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":2,\"totalTokenCount\":3}}\n\n",
            ),
            &mut buffer,
        ));
        assert_eq!(usage.as_valid().map(|usage| usage.total_tokens), Some(3));

        usage.observe(extract_gemini_sse_usage(
            &Bytes::from_static(b"data: {\"candidates\":[]}\n\ndata: [DONE]\n\n"),
            &mut buffer,
        ));
        assert_eq!(
            usage.as_valid().map(|usage| usage.total_tokens),
            Some(3),
            "events without usageMetadata must preserve cumulative usage"
        );

        usage.observe(extract_gemini_sse_usage(
            &Bytes::from_static(
                b"data: {\"usageMetadata\":{\"promptTokenCount\":4,\"totalTokenCount\":6}}\n\n",
            ),
            &mut buffer,
        ));
        assert!(matches!(usage, GeminiStreamUsage::Invalid));

        usage.observe(extract_gemini_sse_usage(
            &Bytes::from_static(
                b"data: {\"usageMetadata\":{\"promptTokenCount\":4,\"candidatesTokenCount\":2,\"totalTokenCount\":6}}\n\n",
            ),
            &mut buffer,
        ));
        assert_eq!(
            usage.as_valid().map(|usage| usage.total_tokens),
            Some(6),
            "a later authoritative cumulative usage event may recover the stream"
        );
    }

    #[test]
    fn native_sse_usage_rejects_later_malformed_or_truncated_events() {
        let mut buffer = String::new();
        let usage = extract_gemini_sse_usage(
            &Bytes::from_static(
                b"data: {\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":2,\"totalTokenCount\":3}}\n\ndata: {\"usageMetadata\":{\"promptTokenCount\":4,\"totalTokenCount\":6}}\n\n",
            ),
            &mut buffer,
        );
        assert!(matches!(usage, GeminiStreamUsage::Invalid));

        let mut usage = GeminiStreamUsage::Valid(PricingUsage::new(1, 2));
        usage.observe(extract_gemini_sse_usage(
            &Bytes::from_static(b"data: {\"usageMetadata\":{\"promptTokenCount\":4"),
            &mut buffer,
        ));
        assert_eq!(
            usage.as_valid().map(|usage| usage.total_tokens),
            Some(3),
            "an incomplete buffered event is not invalid until the stream ends"
        );
        usage.observe(finish_gemini_sse_usage(&mut buffer));
        assert!(matches!(usage, GeminiStreamUsage::Invalid));
        assert!(buffer.is_empty());
    }

    #[test]
    fn native_usage_parser_preserves_effective_cache_without_reasoning_charge() {
        let value = serde_json::json!({"usageMetadata": {
            "promptTokenCount": 10, "toolUsePromptTokenCount": 2,
            "candidatesTokenCount": 3, "thoughtsTokenCount": 4,
            "cachedContentTokenCount": 5, "totalTokenCount": 17
        }});
        let usage = gemini_usage_metadata(&value).unwrap();
        assert_eq!(
            (
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens
            ),
            (12, 7, 19)
        );
        assert_eq!(usage.cached_tokens, Some(5));
        assert_eq!(usage.cache_read_tokens, Some(5));
        assert_eq!(usage.reasoning_tokens, None);
        for bad in [
            serde_json::json!({"usageMetadata": {"promptTokenCount": 2, "candidatesTokenCount": 1, "totalTokenCount": 4}}),
            serde_json::json!({"usageMetadata": {"promptTokenCount": 2, "candidatesTokenCount": "1", "totalTokenCount": 3}}),
            serde_json::json!({"usageMetadata": {"promptTokenCount": 0, "candidatesTokenCount": 0, "totalTokenCount": 0}}),
        ] {
            assert!(gemini_usage_metadata(&bad).is_none());
        }
        let huge = serde_json::json!({"usageMetadata": {
            "promptTokenCount": u64::MAX, "candidatesTokenCount": 0,
            "totalTokenCount": u64::MAX
        }});
        assert_eq!(gemini_usage_metadata(&huge).unwrap().total_tokens, u32::MAX);
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

    async fn assert_native_no_usage_case(
        provider_amount: Option<f64>,
        key_amount: Option<f64>,
        key_budget_enabled: bool,
    ) {
        let pricing = PricingService::with_embedded_default().expect("pricing");
        let pricing_config = GatewayPricingConfig::default();
        let budget_limits = UnifiedBudgetLimits::new();
        let provider = test_gemini_route_provider("gemini", "vertex_ai", "gemini-test");
        budget_limits.providers.set_provider_limit(
            &provider.provider_name,
            ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
        );
        budget_limits.models.set_model_limit(
            &provider.model,
            ModelLimitConfig::new(10.0, ResetPeriod::Monthly),
        );
        let provider_reservation = provider_amount.map(|amount| {
            budget_limits
                .reserve_spend(&provider.provider_name, &provider.model, amount)
                .expect("provider reservation")
        });
        let budget_manager = BudgetManager::new();
        let scope = BudgetScope::ApiKey("native-no-usage".to_string());
        let mut key_budget_config = BudgetConfig::new("native no usage", 10.0);
        key_budget_config.enabled = Some(key_budget_enabled);
        budget_manager
            .create_budget(scope.clone(), key_budget_config)
            .await
            .expect("key budget");
        let key_reservation = key_amount.map(|amount| {
            budget_manager
                .tracker()
                .reserve_spend(&scope, amount)
                .expect("key reservation")
        });
        let key_reserved = key_reservation
            .as_ref()
            .map(BudgetReservation::reserved_amount);
        let has_key_reservation = key_reservation.is_some();
        let key_manager = KeyManager::new(InMemoryKeyRepository::new());
        let (key_id, _) = key_manager
            .generate_key(CreateKeyConfig {
                name: "native matrix key".to_string(),
                ..Default::default()
            })
            .await
            .expect("API key");
        let state = GeminiSpendState {
            pricing: &pricing,
            pricing_config: &pricing_config,
            budget_limits: &budget_limits,
            key_manager: &key_manager,
            api_key_id: Some(key_id),
        };
        settle_gemini_stream_spend(
            &state,
            &provider,
            GeminiStreamUsage::Invalid,
            provider_reservation,
            key_reservation,
            true,
        )
        .await;

        let provider_spend = budget_limits
            .providers
            .get_provider_usage(&provider.provider_name)
            .expect("provider usage")
            .current_spend;
        let model_spend = budget_limits
            .models
            .get_model_usage(&provider.model)
            .expect("model usage")
            .current_spend;
        let expected_provider = provider_amount.unwrap_or(0.0);
        assert!((provider_spend - expected_provider).abs() < f64::EPSILON);
        assert!((model_spend - expected_provider).abs() < f64::EPSILON);
        let expected_cost = key_reserved
            .filter(|amount| *amount > 0.0)
            .or_else(|| provider_amount.filter(|amount| *amount > 0.0));
        let expected_key_spend = if has_key_reservation {
            expected_cost.unwrap_or(0.0)
        } else {
            0.0
        };
        assert!(
            (budget_manager.get_current_spend(&scope) - expected_key_spend).abs() < f64::EPSILON
        );
        let stats = key_manager
            .get_usage_stats(key_id)
            .await
            .expect("usage stats");
        assert_eq!(stats.total_requests, u64::from(expected_cost.is_some()));
        assert!((stats.total_cost - expected_cost.unwrap_or(0.0)).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn invalid_stream_usage_settles_each_reservation_own_amount() {
        assert_native_no_usage_case(Some(0.4), Some(0.2), true).await;
        assert_native_no_usage_case(Some(0.4), None, true).await;
        assert_native_no_usage_case(None, Some(0.2), true).await;
        assert_native_no_usage_case(None, None, true).await;
        assert_native_no_usage_case(Some(0.4), Some(0.2), false).await;
        assert_native_no_usage_case(None, Some(0.2), false).await;
    }
}
