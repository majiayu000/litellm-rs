//! Audio speech endpoint (text-to-speech)

use crate::core::audio::types::SpeechRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde::Deserialize;
use tracing::{error, info};

use super::super::budgeted::{ApiKeyBudgetPolicy, run_unary};
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
    /// Streaming speech is not supported; `true` fails closed.
    #[serde(default)]
    pub stream: Option<bool>,
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

    if request.stream == Some(true) {
        return Ok(openai_errors::validation_error(
            "Streaming speech is not supported",
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
    let budgeted = state.budgeted.clone();
    let key_manager = budgeted.key_manager();
    let pricing_service = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();

    match run_unary(
        &state.unified_router,
        &requested_model,
        ProviderCapability::TextToSpeech,
        move |provider, selected_model, _deployment_id| {
            let mut request = speech_request.clone();
            let context = context_for_execution.clone();
            let budgeted = budgeted.clone();
            let key_manager = key_manager.clone();
            let pricing_service = pricing_service.clone();
            let pricing_config = pricing_config.clone();
            async move {
                let usage = super::budgeting::speech_usage(&request.input);
                let budget_provider = provider.name().to_string();
                let request_pricing = super::super::spend::request_pricing_for_provider(
                    &pricing_service,
                    &provider,
                    &selected_model,
                    ProviderCapability::TextToSpeech,
                )?;
                let pricing_units = request_pricing.has_character_pricing().then(|| {
                    super::budgeting::AudioPricingUnits::Characters(
                        request.input.chars().count() as f64
                    )
                });
                request.model = selected_model.clone();
                let reserve_pricing_config = pricing_config.clone();
                let settle_pricing_config = pricing_config;
                let reserve_request_pricing = request_pricing.clone();
                let settle_request_pricing = request_pricing;
                let reserve_pricing_units = pricing_units.clone();
                let settle_pricing_units = pricing_units;
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
                            super::budgeting::reserve_audio_provider_budget_with_pricing(
                                &reserve_request_pricing,
                                &reserve_pricing_config,
                                budget.budget_limits(),
                                budget.provider(),
                                budget.model(),
                                reserve_pricing_units,
                                &reserve_usage,
                            )
                        },
                        || provider.text_to_speech(request, context),
                        |response, reservations, budget| {
                            let (budget_reservation, key_budget_reservation) =
                                reservations.into_parts();
                            async move {
                                let tokens_used = u64::from(settle_usage.total_tokens);
                                super::budgeting::record_audio_spend(
                                    &settle_request_pricing,
                                    &settle_pricing_config,
                                    budget.budget_limits(),
                                    &settle_key_manager,
                                    api_key_id,
                                    budget.provider(),
                                    budget.model(),
                                    settle_pricing_units,
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
