//! Audio translations endpoint

use crate::core::audio::AudioService;
use crate::core::audio::types::TranslationRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use actix_multipart::Multipart;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use futures::StreamExt;
use tracing::{error, info};

use super::upload::{drain_field, read_audio_file, read_text_field, upload_error_response};
use crate::server::routes::ai::context::get_request_context;
use crate::server::routes::ai::openai_errors;
use crate::server::routes::ai::provider_selection::select_provider_for_model;

/// Audio translations endpoint
///
/// OpenAI-compatible audio translation API.
/// Translates audio to English text.
pub async fn audio_translations(
    state: web::Data<AppState>,
    req: HttpRequest,
    mut payload: Multipart,
) -> ActixResult<HttpResponse> {
    info!("Audio translations request");

    // Get request context (validates auth)
    let _context = match get_request_context(&req) {
        Ok(ctx) => ctx,
        Err(_) => {
            return Ok(openai_errors::unauthorized_error("Unauthorized"));
        }
    };

    // Parse multipart form data (similar to transcriptions)
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename = String::from("audio.mp3");
    let mut model = String::from("whisper-large-v3-turbo");
    let mut prompt: Option<String> = None;
    let mut response_format: Option<String> = None;
    let mut temperature: Option<f32> = None;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => {
                error!("Error reading multipart field: {}", e);
                return Ok(openai_errors::validation_error(format!(
                    "Invalid multipart data: {}",
                    e
                )));
            }
        };

        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => {
                if let Err(e) = drain_field(&mut field).await {
                    return Ok(upload_error_response(e));
                }
                continue;
            }
        };

        match field_name.as_str() {
            "file" => {
                if let Some(cd) = field.content_disposition()
                    && let Some(fname) = cd.get_filename()
                {
                    filename = fname.to_string();
                }
                let data = match read_audio_file(&mut field).await {
                    Ok(data) => data,
                    Err(e) => return Ok(upload_error_response(e)),
                };
                file_data = Some(data);
            }
            "model" => match read_text_field(&mut field).await {
                Ok(value) if !value.is_empty() => model = value,
                Ok(_) => {}
                Err(e) => return Ok(upload_error_response(e)),
            },
            "prompt" => match read_text_field(&mut field).await {
                Ok(value) if !value.is_empty() => prompt = Some(value),
                Ok(_) => {}
                Err(e) => return Ok(upload_error_response(e)),
            },
            "response_format" => match read_text_field(&mut field).await {
                Ok(value) if !value.is_empty() => response_format = Some(value),
                Ok(_) => {}
                Err(e) => return Ok(upload_error_response(e)),
            },
            "temperature" => match read_text_field(&mut field).await {
                Ok(value) => {
                    if let Ok(temp) = value.parse::<f32>() {
                        temperature = Some(temp);
                    }
                }
                Err(e) => return Ok(upload_error_response(e)),
            },
            _ => {
                if let Err(e) = drain_field(&mut field).await {
                    return Ok(upload_error_response(e));
                }
            }
        }
    }

    let file = match file_data {
        Some(data) if !data.is_empty() => data,
        _ => {
            return Ok(openai_errors::validation_error("No audio file provided"));
        }
    };

    let unified_router = &state.unified_router;

    let selected_model = match select_provider_for_model(
        unified_router,
        &model,
        ProviderCapability::AudioTranslation,
    ) {
        Ok(selection) => selection,
        Err(e) => return Ok(openai_errors::gateway_error_response(&e)),
    };

    let translation_request = TranslationRequest {
        file,
        filename,
        model: selected_model,
        prompt,
        response_format,
        temperature,
    };

    let audio_service = AudioService::new();

    match audio_service.translate(translation_request).await {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(e) => {
            error!("Translation error: {}", e);
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}
