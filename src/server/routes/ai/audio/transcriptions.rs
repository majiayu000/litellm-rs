//! Audio transcriptions endpoint

use crate::core::audio::AudioService;
use crate::core::audio::types::TranscriptionRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_multipart::Multipart;
use actix_web::{HttpRequest, HttpResponse, ResponseError, Result as ActixResult, web};
use futures::StreamExt;
use tracing::{error, info};

use crate::server::routes::ai::context::get_request_context;
use crate::server::routes::ai::provider_selection::select_provider_for_model;

/// Audio transcriptions endpoint
///
/// OpenAI-compatible audio transcription API (Whisper).
/// Accepts multipart/form-data with audio file.
pub async fn audio_transcriptions(
    state: web::Data<AppState>,
    req: HttpRequest,
    mut payload: Multipart,
) -> ActixResult<HttpResponse> {
    info!("Audio transcriptions request");

    // Get request context (validates auth)
    let _context = match get_request_context(&req) {
        Ok(ctx) => ctx,
        Err(_) => {
            return Ok(HttpResponse::Unauthorized()
                .json(ApiResponse::<()>::error("Unauthorized".to_string())));
        }
    };

    // Parse multipart form data
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename = String::from("audio.mp3");
    let mut model = String::from("whisper-large-v3-turbo");
    let mut language: Option<String> = None;
    let mut prompt: Option<String> = None;
    let mut response_format: Option<String> = None;
    let mut temperature: Option<f32> = None;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => {
                error!("Error reading multipart field: {}", e);
                return Ok(
                    HttpResponse::BadRequest().json(ApiResponse::<()>::error(format!(
                        "Invalid multipart data: {}",
                        e
                    ))),
                );
            }
        };

        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => continue,
        };

        match field_name.as_str() {
            "file" => {
                // Get filename from content disposition
                if let Some(cd) = field.content_disposition()
                    && let Some(fname) = cd.get_filename()
                {
                    filename = fname.to_string();
                }

                // Read file data
                let mut data = Vec::new();
                while let Some(chunk) = field.next().await {
                    match chunk {
                        Ok(bytes) => data.extend_from_slice(&bytes),
                        Err(e) => {
                            error!("Error reading file chunk: {}", e);
                            return Ok(HttpResponse::BadRequest()
                                .json(ApiResponse::<()>::error("Error reading file".to_string())));
                        }
                    }
                }
                file_data = Some(data);
            }
            "model" => {
                if let Some(Ok(bytes)) = field.next().await {
                    model = String::from_utf8_lossy(&bytes).to_string();
                }
            }
            "language" => {
                if let Some(Ok(bytes)) = field.next().await {
                    language = Some(String::from_utf8_lossy(&bytes).to_string());
                }
            }
            "prompt" => {
                if let Some(Ok(bytes)) = field.next().await {
                    prompt = Some(String::from_utf8_lossy(&bytes).to_string());
                }
            }
            "response_format" => {
                if let Some(Ok(bytes)) = field.next().await {
                    response_format = Some(String::from_utf8_lossy(&bytes).to_string());
                }
            }
            "temperature" => {
                if let Some(Ok(bytes)) = field.next().await
                    && let Ok(temp) = String::from_utf8_lossy(&bytes).parse::<f32>()
                {
                    temperature = Some(temp);
                }
            }
            _ => {
                // Skip unknown fields
                while field.next().await.is_some() {}
            }
        }
    }

    // Validate file was provided
    let file = match file_data {
        Some(data) if !data.is_empty() => data,
        _ => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                "No audio file provided".to_string(),
            )));
        }
    };

    let unified_router = &state.unified_router;

    let selected_model = match select_provider_for_model(
        unified_router,
        &model,
        ProviderCapability::AudioTranscription,
    ) {
        Ok(selection) => selection,
        Err(e) => return Ok(e.error_response()),
    };

    // Create transcription request
    let transcription_request = TranscriptionRequest {
        file,
        filename,
        model: selected_model,
        language,
        prompt,
        response_format,
        temperature,
        timestamp_granularities: None,
    };

    // Create audio service and process request
    let audio_service = AudioService::new();

    match audio_service.transcribe(transcription_request).await {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(e) => {
            error!("Transcription error: {}", e);
            Ok(e.error_response())
        }
    }
}
