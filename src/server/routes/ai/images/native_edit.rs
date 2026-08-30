use actix_web::HttpResponse;
use bytes::Bytes;

use crate::core::pricing_service::PricingUsage;
use crate::core::providers::Provider;
use crate::core::types::context::RequestContext;
use crate::core::types::image::ImageEditRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::super::budgeted::{ApiKeyBudgetPolicy, run_unary};
use super::multipart::{extract_file_field, extract_text_field};

pub(super) fn is_native_image_edit_selected(state: &AppState, model: &str) -> bool {
    let Some(deployment) = state
        .unified_router
        .select_capability_deployment(model, &ProviderCapability::ImageEdit)
    else {
        return false;
    };

    is_native_image_provider(&deployment.provider)
}

#[cfg(feature = "providers-extended")]
fn is_native_image_provider(provider: &Provider) -> bool {
    matches!(
        provider,
        Provider::Stability(_) | Provider::BlackForestLabs(_)
    )
}

#[cfg(not(feature = "providers-extended"))]
fn is_native_image_provider(_provider: &Provider) -> bool {
    false
}

pub(super) async fn handle_native_image_edit(
    state: &AppState,
    context: RequestContext,
    body: &Bytes,
    content_type: &str,
    requested_model: &str,
) -> Result<HttpResponse, GatewayError> {
    let request = parse_native_image_edit(body, content_type, requested_model)?;
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budgeted = state.budgeted.clone();
    let key_manager = budgeted.key_manager();
    let pricing_service = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let context_for_execution = context.clone();

    let response = run_unary(
        &state.unified_router,
        requested_model,
        ProviderCapability::ImageEdit,
        move |provider, selected_model, _deployment_id| {
            let mut request = request.clone();
            let context = context_for_execution.clone();
            let budgeted = budgeted.clone();
            let key_manager = key_manager.clone();
            let pricing_service = pricing_service.clone();
            let pricing_config = pricing_config.clone();
            async move {
                let budget_provider = provider.name().to_string();
                let mut request_pricing = super::super::spend::request_pricing_for_provider(
                    &pricing_service,
                    &provider,
                    &selected_model,
                    ProviderCapability::ImageEdit,
                )?;
                if let Some(variant) = super::pricing_keys::resolve_image_request_pricing(
                    &request_pricing,
                    request.size.as_deref(),
                    None,
                ) {
                    request_pricing = variant;
                }
                let usage = estimated_image_edit_usage(&request, &request_pricing);
                request.model = Some(selected_model.clone());
                let reserve_pricing_config = pricing_config.clone();
                let settle_pricing_config = pricing_config;
                let reserve_request_pricing = request_pricing.clone();
                let settle_request_pricing = request_pricing;
                let reserve_usage = usage.clone();
                let settle_usage = usage;
                let settle_key_manager = key_manager.clone();

                budgeted
                    .for_selected_with_api_key_budget(
                        budget_provider,
                        selected_model,
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
                        || provider.edit_image(request, context),
                        |response, reservations, budget| {
                            let (budget_reservation, key_budget_reservation) =
                                reservations.into_parts();
                            async move {
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
                                let tokens_used = u64::from(
                                    settle_usage.total_tokens.saturating_add(
                                        settle_usage.image_tokens.unwrap_or(0),
                                    ),
                                );
                                (response, tokens_used)
                            }
                        },
                    )
                    .await
            }
        },
    )
    .await?;

    Ok(HttpResponse::Ok().json(response))
}

fn parse_native_image_edit(
    body: &Bytes,
    content_type: &str,
    model: &str,
) -> Result<ImageEditRequest, GatewayError> {
    let image = extract_file_field(body, content_type, "image")
        .filter(|image| !image.is_empty())
        .ok_or_else(|| GatewayError::validation("image is required"))?;
    let prompt = extract_text_field(body, content_type, "prompt")
        .filter(|prompt| !prompt.is_empty())
        .ok_or_else(|| GatewayError::validation("prompt is required"))?;
    let n = extract_text_field(body, content_type, "n")
        .map(|value| {
            value
                .parse::<u32>()
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| GatewayError::validation("n must be a positive integer"))
        })
        .transpose()?;

    Ok(ImageEditRequest {
        image,
        mask: extract_file_field(body, content_type, "mask"),
        prompt,
        model: Some(model.to_string()),
        n,
        size: extract_text_field(body, content_type, "size"),
        response_format: extract_text_field(body, content_type, "response_format"),
        user: extract_text_field(body, content_type, "user"),
    })
}

fn estimated_image_edit_usage(
    request: &ImageEditRequest,
    request_pricing: &super::super::spend::RequestPricing,
) -> PricingUsage {
    let image_count = request.n.unwrap_or(1);
    let mut usage = PricingUsage::new(super::estimated_text_tokens(&request.prompt), 0);
    usage.image_tokens = Some(super::estimated_image_output_tokens(
        request.size.as_deref(),
        None,
        image_count,
    ));
    usage.output_image_count = Some(image_count.max(1));
    usage.output_image_pricing_keys = request_pricing
        .priced_parts()
        .map(|(provider, model)| {
            super::pricing_keys::image_pricing_keys(provider, model, request.size.as_deref(), None)
        })
        .unwrap_or_default();
    usage
}
