//! Audio speech endpoint (text-to-speech)

use crate::core::audio::types::SpeechRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde::Deserialize;
use tracing::{error, info};

use super::super::execution::execute_with_selected_deployment;
use crate::server::routes::ai::context::{
    enforce_api_key_model_and_token_limits, get_request_context,
};
use crate::server::routes::ai::openai_errors;

/// Audio speech generation request
#[derive(Debug, Deserialize)]
pub struct AudioSpeechRequest {
    /// Text to convert to speech
    pub input: String,
    /// Model to use
    #[serde(default = "default_tts_model")]
    pub model: String,
    /// Voice to use for speech generation
    pub voice: String,
    /// Audio format (mp3, opus, aac, flac)
    pub response_format: Option<String>,
    /// Speed of speech (0.25 to 4.0)
    pub speed: Option<f32>,
}

fn default_tts_model() -> String {
    "tts-1".to_string()
}

/// Audio speech endpoint
///
/// OpenAI-compatible text-to-speech API.
pub async fn audio_speech(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<AudioSpeechRequest>,
) -> ActixResult<HttpResponse> {
    info!(
        "Audio speech request: model={}, voice={}, text_len={}",
        request.model,
        request.voice,
        request.input.len()
    );

    // Get request context (validates auth)
    let context = match get_request_context(&req) {
        Ok(ctx) => ctx,
        Err(_) => {
            return Ok(openai_errors::unauthorized_error("Unauthorized"));
        }
    };

    if request.input.len() > 4096 {
        return Ok(openai_errors::validation_error(
            "Input text too long (max 4096 characters)",
        ));
    }

    if let Err(error) = enforce_api_key_model_and_token_limits(&req, &request.model, None) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let speech_request = SpeechRequest {
        input: request.input.clone(),
        model: request.model.clone(),
        voice: request.voice.clone(),
        response_format: request.response_format.clone(),
        speed: request.speed,
    };

    let requested_model = request.model.clone();
    let context_for_execution = context.clone();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budget_manager = state.budget_manager.clone();
    let budget_limits = state.budget_limits.clone();
    let key_manager = state.key_manager.clone();
    let pricing_service = state.pricing.clone();
    let pricing_config = state.config().gateway.pricing.clone();

    match execute_with_selected_deployment(
        &state.unified_router,
        &requested_model,
        ProviderCapability::TextToSpeech,
        move |provider, selected_model, _deployment_id| {
            let mut request = speech_request.clone();
            let context = context_for_execution.clone();
            let budget_manager = budget_manager.clone();
            let budget_limits = budget_limits.clone();
            let key_manager = key_manager.clone();
            let pricing_service = pricing_service.clone();
            let pricing_config = pricing_config.clone();
            async move {
                let usage = super::budgeting::speech_usage(&request.input);
                let budget_provider = provider.name().to_string();
                let (pricing_provider, pricing_model) =
                    super::super::spend::pricing_identity_for_provider(
                        pricing_service.as_ref(),
                        &provider,
                        &selected_model,
                    );
                let (budget_reservation, key_budget_reservation) =
                    super::budgeting::reserve_audio_budget_with_pricing(
                        pricing_service.as_ref(),
                        &pricing_config,
                        &budget_manager,
                        &budget_limits,
                        api_key_budget_id,
                        &budget_provider,
                        &selected_model,
                        &pricing_provider,
                        &pricing_model,
                        None,
                        &usage,
                    )?;
                request.model = selected_model.clone();
                let response = provider.text_to_speech(request, context).await?;
                let tokens_used = u64::from(usage.total_tokens);
                super::budgeting::record_audio_spend(
                    pricing_service.as_ref(),
                    &pricing_config,
                    &budget_limits,
                    &key_manager,
                    api_key_id,
                    &budget_provider,
                    &selected_model,
                    &pricing_provider,
                    &pricing_model,
                    None,
                    &usage,
                    budget_reservation,
                    key_budget_reservation,
                )
                .await;
                Ok((response, tokens_used))
            }
        },
    )
    .await
    {
        Ok(response) => Ok(HttpResponse::Ok()
            .content_type(response.content_type)
            .body(response.audio)),
        Err(e) => {
            error!("Speech generation error: {}", e);
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}
