use bytes::Bytes;

use crate::core::pricing_service::PricingUsage;
use crate::core::providers::{Provider, ProviderError};
use crate::core::types::context::RequestContext;
use crate::core::types::image::ImageEditRequest;
use crate::core::types::responses::ImageGenerationResponse;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::super::budgeted::ApiKeyBudgetPolicy;
use super::multipart::{extract_file_fields, extract_text_field};

#[cfg(feature = "providers-extended")]
pub(super) fn is_native_image_provider(provider: &Provider) -> bool {
    matches!(
        provider,
        Provider::Stability(_) | Provider::BlackForestLabs(_)
    )
}

#[cfg(feature = "providers-extended")]
pub(super) fn deployment_supports_request(
    provider: &Provider,
    selected_model: &str,
    request: Option<&Result<ImageEditRequest, ProviderError>>,
) -> bool {
    if !is_native_image_provider(provider) {
        return true;
    }
    let Some(Ok(request)) = request else {
        return false;
    };
    match provider {
        Provider::Stability(provider) => provider
            .validate_image_edit_request(request, selected_model)
            .is_ok(),
        Provider::BlackForestLabs(provider) => provider
            .validate_image_edit_request(request, selected_model)
            .is_ok(),
        _ => true,
    }
}

#[cfg(not(feature = "providers-extended"))]
pub(super) fn is_native_image_provider(_provider: &Provider) -> bool {
    false
}

#[cfg(not(feature = "providers-extended"))]
pub(super) fn deployment_supports_request(
    _provider: &Provider,
    _selected_model: &str,
    _request: Option<&Result<ImageEditRequest, ProviderError>>,
) -> bool {
    true
}

pub(super) async fn execute_selected_native_image_edit(
    state: &AppState,
    context: RequestContext,
    provider: Provider,
    selected_model: String,
    budget_provider: String,
    mut request: ImageEditRequest,
) -> Result<(ImageGenerationResponse, u64), ProviderError> {
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budgeted = state.budgeted.clone();
    let key_manager = budgeted.key_manager();
    let pricing_service = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let mut request_pricing = super::super::spend::request_pricing_for_provider(
        &pricing_service,
        &provider,
        &selected_model,
        crate::core::types::model::ProviderCapability::ImageEdit,
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

    super::native_spend::reserve_call_settle_media_job(
        budgeted.for_selected_with_api_key_budget(
            budget_provider,
            selected_model,
            api_key_budget_id,
            ApiKeyBudgetPolicy::FromProviderReservation,
        ),
        async |budget| {
            super::super::spend::reserve_pricing_usage_budget_with_request_pricing(
                &reserve_request_pricing,
                &reserve_pricing_config,
                budget.budget_limits(),
                budget.provider(),
                budget.model(),
                &reserve_usage,
            )
            .await
        },
        || provider.edit_image(request, context),
        |reservations, budget| async move {
            let (budget_reservation, key_budget_reservation) = reservations.into_parts();
            super::super::execution::completion::observe_unknown_usage();
            let tokens_used = super::super::execution::estimate::usage(&settle_usage);
            super::super::spend::record_pricing_usage_spend_with_admission(
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
                tokens_used,
            )
            .await;
            0
        },
    )
    .await
}

pub(super) fn parse_native_image_edit(
    body: &Bytes,
    content_type: &str,
    model: &str,
) -> Result<ImageEditRequest, GatewayError> {
    let mut images = extract_file_fields(body, content_type, "image").unwrap_or_default();
    if images.len() > 1 {
        return Err(GatewayError::validation(
            "native image editing accepts exactly one image",
        ));
    }
    let image = images
        .pop()
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
    let mut masks = extract_file_fields(body, content_type, "mask").unwrap_or_default();
    if masks.len() > 1 {
        return Err(GatewayError::validation(
            "native image editing accepts at most one mask",
        ));
    }

    Ok(ImageEditRequest {
        image,
        mask: masks.pop(),
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
    let prompt_tokens = super::estimated_text_tokens(&request.prompt)
        .saturating_add(super::super::spend::uploaded_image_tokens(&request.image))
        .saturating_add(
            request
                .mask
                .as_deref()
                .map(super::super::spend::uploaded_image_tokens)
                .unwrap_or(0),
        );
    let mut usage = PricingUsage::new(prompt_tokens, 0);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_image_edit_usage_counts_uploaded_image_and_mask() {
        let pricing = crate::core::pricing_service::PricingService::new(None);
        let pricing = super::super::super::spend::RequestPricing::from_exact(
            &pricing,
            "openai",
            "gpt-image-1-mini",
        );
        let mut request = ImageEditRequest {
            image: vec![0; 9000],
            mask: Some(vec![0; 6000]),
            prompt: String::new(),
            model: None,
            n: None,
            size: None,
            response_format: None,
            user: None,
        };
        let with_mask = estimated_image_edit_usage(&request, &pricing);
        assert_eq!(with_mask.prompt_tokens, 1 + 3000 + 2000);
        request.mask = None;
        let without_mask = estimated_image_edit_usage(&request, &pricing);
        assert_eq!(without_mask.prompt_tokens, 1 + 3000);
        assert_eq!(with_mask.image_tokens, without_mask.image_tokens);
        assert_eq!(with_mask.output_image_count, Some(1));
    }
}
