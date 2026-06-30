use crate::core::models::openai::{ImageGenerationRequest, ImageGenerationResponse};
use crate::core::pricing_service::PricingUsage;
use crate::core::types::context::RequestContext;
use crate::core::types::image::ImageGenerationRequest as CoreImageRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::super::execution::execute_with_selected_deployment;

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
        quality: None,
        style: None,
    };

    let context_for_execution = context.clone();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budget_manager = state.budget_manager.clone();
    let budget_limits = state.budget_limits.clone();
    let key_manager = state.key_manager.clone();
    let pricing_service = state.pricing.clone();
    let core_response = execute_with_selected_deployment(
        &state.unified_router,
        &requested_model,
        ProviderCapability::ImageGeneration,
        move |provider, selected_model, _deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            let budget_manager = budget_manager.clone();
            let budget_limits = budget_limits.clone();
            let key_manager = key_manager.clone();
            let pricing_service = pricing_service.clone();
            async move {
                super::super::spend::ensure_budget_available(
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                )?;
                let budget_provider = provider.name().to_string();
                let (pricing_provider, pricing_model) =
                    super::super::spend::pricing_identity_for_provider(
                        pricing_service.as_ref(),
                        &provider,
                        &selected_model,
                    );
                let usage = estimated_image_generation_usage(&core_request);
                let budget_reservation =
                    super::super::spend::reserve_pricing_usage_budget_with_pricing(
                        pricing_service.as_ref(),
                        &budget_limits,
                        &budget_provider,
                        &selected_model,
                        &pricing_provider,
                        &pricing_model,
                        &usage,
                    )?;
                let key_budget_reservation =
                    super::super::spend::reserve_api_key_budget_for_reservation(
                        &budget_manager,
                        api_key_budget_id,
                        budget_reservation.as_ref(),
                    )?;
                let mut request_for_provider = core_request.clone();
                request_for_provider.model = Some(selected_model.clone());
                let response = provider
                    .create_images(request_for_provider, context)
                    .await?;
                let tokens_used = u64::from(
                    usage
                        .total_tokens
                        .saturating_add(usage.image_tokens.unwrap_or(0)),
                );
                super::super::spend::record_pricing_usage_spend_with_reservation_with_pricing(
                    pricing_service.as_ref(),
                    &budget_limits,
                    &key_manager,
                    api_key_id,
                    &budget_provider,
                    &selected_model,
                    &pricing_provider,
                    &pricing_model,
                    &usage,
                    budget_reservation,
                    key_budget_reservation,
                )
                .await;
                Ok((response, tokens_used))
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

fn estimated_image_generation_usage(request: &CoreImageRequest) -> PricingUsage {
    let prompt_tokens = super::estimated_text_tokens(&request.prompt);
    let image_tokens = super::estimated_image_output_tokens(
        request.size.as_deref(),
        request.quality.as_deref(),
        request.n.unwrap_or(1),
    );
    let mut usage = PricingUsage::new(prompt_tokens, 0);
    usage.image_tokens = Some(image_tokens);
    usage
}
