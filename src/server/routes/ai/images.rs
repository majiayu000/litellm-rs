//! Image generation endpoint

use crate::core::models::openai::{ImageGenerationRequest, ImageGenerationResponse};
use crate::core::types::context::RequestContext;
use crate::core::types::image::ImageGenerationRequest as CoreImageRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::info;

use super::context::handle_ai_request;
use super::execution::execute_with_selected_deployment;
use super::provider_selection::select_provider_for_optional_model;

/// Image generation endpoint
///
/// OpenAI-compatible image generation API.
pub async fn image_generations(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ImageGenerationRequest>,
) -> ActixResult<HttpResponse> {
    info!("Image generation request for model: {:?}", request.model);

    handle_ai_request(
        &req,
        request.into_inner(),
        "Image generation",
        |request, context| handle_image_generation_with_state(state.get_ref(), request, context),
    )
    .await
}

/// Handle image generation with app state (UnifiedRouter only)
pub async fn handle_image_generation_with_state(
    state: &AppState,
    request: ImageGenerationRequest,
    context: RequestContext,
) -> Result<ImageGenerationResponse, GatewayError> {
    let unified_router = &state.unified_router;
    handle_image_generation_internal(unified_router, request, context).await
}

async fn handle_image_generation_internal(
    unified_router: &crate::core::router::UnifiedRouter,
    request: ImageGenerationRequest,
    context: RequestContext,
) -> Result<ImageGenerationResponse, GatewayError> {
    let selected_model = select_provider_for_optional_model(
        unified_router,
        request.model.as_deref(),
        ProviderCapability::ImageGeneration,
    )?;

    let core_request = CoreImageRequest {
        prompt: request.prompt,
        model: Some(selected_model.clone()),
        n: request.n,
        size: request.size,
        response_format: request.response_format,
        user: request.user,
        quality: None,
        style: None,
    };

    let context_for_execution = context.clone();
    let core_response = execute_with_selected_deployment(
        unified_router,
        &selected_model,
        move |provider, selected_model| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            async move {
                let mut request_for_provider = core_request.clone();
                request_for_provider.model = Some(selected_model);
                let response = provider
                    .create_images(request_for_provider, context)
                    .await?;
                Ok((response, 0))
            }
        },
    )
    .await?;

    // Convert core response to OpenAI format
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
