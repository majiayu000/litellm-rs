//! Audio speech endpoint (text-to-speech)

use crate::core::audio::types::SpeechRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde::Deserialize;
use tracing::{error, info};

use super::super::execution::execute_with_selected_deployment;
use crate::server::routes::ai::context::get_request_context;
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

    let speech_request = SpeechRequest {
        input: request.input.clone(),
        model: request.model.clone(),
        voice: request.voice.clone(),
        response_format: request.response_format.clone(),
        speed: request.speed,
    };

    let requested_model = request.model.clone();
    let context_for_execution = context.clone();

    match execute_with_selected_deployment(
        &state.unified_router,
        &requested_model,
        ProviderCapability::TextToSpeech,
        move |provider, selected_model, _deployment_id| {
            let mut request = speech_request.clone();
            let context = context_for_execution.clone();
            async move {
                request.model = selected_model;
                let response = provider.text_to_speech(request, context).await?;
                Ok((response, 0))
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
