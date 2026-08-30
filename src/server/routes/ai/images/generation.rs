use crate::core::models::openai::{ImageGenerationRequest, ImageGenerationResponse};
use crate::core::pricing_service::PricingUsage;
use crate::core::types::context::RequestContext;
use crate::core::types::image::ImageGenerationRequest as CoreImageRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::super::budgeted::{ApiKeyBudgetPolicy, run_unary};

/// Handle image generation with app state (UnifiedRouter only)
pub async fn handle_image_generation_with_state(
    state: &AppState,
    request: ImageGenerationRequest,
    context: RequestContext,
) -> Result<ImageGenerationResponse, GatewayError> {
    let requested_model = request
        .model
        .clone()
        .ok_or_else(|| GatewayError::validation("Model is required"))?;
    if requested_model.trim().is_empty() {
        return Err(GatewayError::validation("Model is required"));
    }

    let core_request = CoreImageRequest {
        prompt: request.prompt,
        model: Some(requested_model.clone()),
        n: request.n,
        size: request.size,
        response_format: request.response_format,
        user: request.user,
        quality: request.quality,
        style: None,
    };

    let context_for_execution = context.clone();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budgeted = state.budgeted.clone();
    let key_manager = budgeted.key_manager();
    let pricing_service = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let core_response = run_unary(
        &state.unified_router,
        &requested_model,
        ProviderCapability::ImageGeneration,
        move |provider, selected_model, _deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            let budgeted = budgeted.clone();
            let key_manager = key_manager.clone();
            let pricing_service = pricing_service.clone();
            let pricing_config = pricing_config.clone();
            async move {
                let budget_provider = provider.name().to_string();
                let mut request_pricing =
                    super::super::spend::request_pricing_for_provider(
                        &pricing_service,
                        &provider,
                        &selected_model,
                        ProviderCapability::ImageGeneration,
                    )?;
                if let Some(variant) = super::pricing_keys::resolve_image_request_pricing(
                    &request_pricing,
                    core_request.size.as_deref(),
                    core_request.quality.as_deref(),
                ) {
                    request_pricing = variant;
                }
                let usage = estimated_image_generation_usage(&core_request, &request_pricing);
                let mut request_for_provider = core_request.clone();
                request_for_provider.model = Some(selected_model.clone());
                let reserve_pricing_config = pricing_config.clone();
                let settle_pricing_config = pricing_config;
                let reserve_request_pricing = request_pricing.clone();
                let settle_request_pricing = request_pricing;
                let reserve_usage = usage.clone();
                let settle_usage = usage;
                let settle_key_manager = key_manager.clone();
                budgeted
                    .for_selected_with_api_key_budget(
                        budget_provider.clone(),
                        selected_model.clone(),
                        api_key_budget_id,
                        ApiKeyBudgetPolicy::FromProviderReservation,
                    )
                    .reserve_call_settle(
                        |budget| {
                            super::super::spend::reserve_pricing_usage_budget_with_request_pricing(
                                &reserve_request_pricing,
                                &reserve_pricing_config,
                                budget.budget_limits(),
                                budget.provider(),
                                budget.model(),
                                &reserve_usage,
                            )
                        },
                        || provider.create_images(request_for_provider, context),
                        |response, reservations, budget| {
                            let (budget_reservation, key_budget_reservation) =
                                reservations.into_parts();
                            async move {
                                let tokens_used = u64::from(
                                    settle_usage
                                        .total_tokens
                                        .saturating_add(settle_usage.image_tokens.unwrap_or(0)),
                                );
                                super::super::spend::record_pricing_usage_spend_with_request_pricing(
                                    &settle_request_pricing,
                                    &settle_pricing_config,
                                    budget.budget_limits(),
                                    &settle_key_manager,
                                    api_key_id,
                                    budget.provider(),
                                    budget.model(),
                                    &settle_usage,
                                    budget_reservation,
                                    key_budget_reservation,
                                )
                                .await;
                                (response, tokens_used)
                            }
                        },
                    )
                    .await
            }
        },
    )
    .await?;

    let response = ImageGenerationResponse {
        created: core_response.created,
        data: core_response
            .data
            .into_iter()
            .map(|d| crate::core::models::openai::ImageObject {
                url: d.url,
                b64_json: d.b64_json,
            })
            .collect(),
    };

    Ok(response)
}

fn estimated_image_generation_usage(
    request: &CoreImageRequest,
    request_pricing: &super::super::spend::RequestPricing,
) -> PricingUsage {
    let prompt_tokens = super::estimated_text_tokens(&request.prompt);
    let image_count = request.n.unwrap_or(1);
    let image_tokens = super::estimated_image_output_tokens(
        request.size.as_deref(),
        request.quality.as_deref(),
        image_count,
    );
    let mut usage = PricingUsage::new(prompt_tokens, 0);
    usage.image_tokens = Some(image_tokens);
    usage.output_image_count = Some(image_count.max(1));
    usage.output_image_pricing_keys = request_pricing
        .priced_parts()
        .map(|(provider, model)| {
            super::pricing_keys::image_pricing_keys(
                provider,
                model,
                request.size.as_deref(),
                request.quality.as_deref(),
            )
        })
        .unwrap_or_default();
    usage
}

#[cfg(test)]
mod tests {
    use crate::core::pricing_service::LiteLLMModelInfo;
    use crate::server::routes::ai::spend::RequestPricing;
    use std::collections::HashMap;

    fn image_model_info(provider: &str, price: f64) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "image_generation".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::from([(
                "output_cost_per_image".to_string(),
                serde_json::Value::from(price),
            )]),
        }
    }

    #[test]
    fn authoritative_pricing_identity_beats_raw_alias_image_variant() {
        let pricing = crate::core::pricing_service::PricingService::new(None);
        pricing.add_custom_model(
            "review-canonical".to_string(),
            image_model_info("review-provider", 0.005),
        );
        pricing.add_custom_model(
            "hd/1024-x-1024/review-public-alias".to_string(),
            image_model_info("review-provider", 0.99),
        );
        pricing.add_custom_model(
            "hd/1024-x-1024/review-canonical".to_string(),
            image_model_info("review-provider", 0.01),
        );

        let request_pricing =
            RequestPricing::from_exact(&pricing, "review-provider", "review-canonical");
        let resolved = super::super::pricing_keys::resolve_image_request_pricing(
            &request_pricing,
            Some("1024x1024"),
            Some("hd"),
        );

        assert_eq!(
            resolved.as_ref().and_then(RequestPricing::priced_parts),
            Some(("review-provider", "hd/1024-x-1024/review-canonical"))
        );
    }

    #[test]
    fn unpriced_authoritative_identity_never_falls_back_to_raw_alias_variant() {
        let pricing = crate::core::pricing_service::PricingService::new(None);
        pricing.add_custom_model(
            "hd/1024-x-1024/review-public-alias".to_string(),
            image_model_info("review-provider", 0.99),
        );

        let request_pricing =
            RequestPricing::from_exact(&pricing, "review-provider", "review-canonical-unpriced");
        let resolved = super::super::pricing_keys::resolve_image_request_pricing(
            &request_pricing,
            Some("1024x1024"),
            Some("hd"),
        );

        assert!(resolved.is_none());
    }
}
