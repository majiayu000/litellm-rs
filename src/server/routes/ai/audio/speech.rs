//! Audio speech endpoint (text-to-speech)

use crate::core::audio::AudioService;
use crate::core::audio::types::SpeechRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, ResponseError, Result as ActixResult, web};
use serde::Deserialize;
use tracing::{error, info};

use crate::server::routes::ai::context::get_request_context;
use crate::server::routes::ai::provider_selection::select_provider_for_model;

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
    let _context = match get_request_context(&req) {
        Ok(ctx) => ctx,
        Err(_) => {
            return Ok(HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::error("Unauthorized".to_string())));
        }
    };

    let unified_router = &state.unified_router;

    let selected_model = match select_provider_for_model(
        unified_router,
        &request.model,
        ProviderCapability::TextToSpeech,
    ) {
        Ok(selection) => selection,
        Err(e) => return Ok(e.error_response()),
    };

    let speech_request = SpeechRequest {
        input: request.input.clone(),
        model: selected_model,
        voice: request.voice.clone(),
        response_format: request.response_format.clone(),
        speed: request.speed,
    };

    let audio_service = AudioService::new();

    match audio_service.speech(speech_request).await {
        Ok(response) => Ok(HttpResponse::Ok()
            .content_type(response.content_type)
            .body(response.audio)),
        Err(e) => {
            error!("Speech generation error: {}", e);
            Ok(e.error_response())
        }
    }
}
