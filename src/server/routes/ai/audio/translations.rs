//! Audio translations endpoint

use crate::core::audio::types::TranslationRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use actix_multipart::Multipart;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use futures::StreamExt;
use tracing::{error, info};

use super::super::execution::execute_with_selected_deployment;
use super::upload::{
    drain_field, parse_optional_f32_field, raw_response_format_error, read_audio_file,
    read_text_field, upload_error_response,
};
use crate::server::routes::ai::context::{
    enforce_api_key_model_and_token_limits, get_request_context,
};
use crate::server::routes::ai::openai_errors;

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
    let context = match get_request_context(&req) {
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
                Ok(value) => match parse_optional_f32_field("temperature", &value) {
                    Ok(parsed) => temperature = parsed,
                    Err(response) => return Ok(response),
                },
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

    if let Some(error_response) = raw_response_format_error(response_format.as_deref()) {
        return Ok(error_response);
    }

    if let Err(error) = enforce_api_key_model_and_token_limits(&req, &model, None) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let translation_request = TranslationRequest {
        file,
        filename,
        model: model.clone(),
        prompt,
        response_format,
        temperature,
    };

    let requested_model = model;
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
        ProviderCapability::AudioTranslation,
        move |provider, selected_model, _deployment_id| {
            let mut request = translation_request.clone();
            let context = context_for_execution.clone();
            let budget_manager = budget_manager.clone();
            let budget_limits = budget_limits.clone();
            let key_manager = key_manager.clone();
            let pricing_service = pricing_service.clone();
            let pricing_config = pricing_config.clone();
            async move {
                let usage =
                    super::budgeting::audio_file_usage(&request.file, request.prompt.as_deref());
                let total_time_seconds =
                    super::budgeting::estimated_audio_file_seconds(&request.file);
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
                        Some(total_time_seconds),
                        &usage,
                    )?;
                request.model = selected_model.clone();
                let response = provider.audio_translation(request, context).await?;
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
                    Some(total_time_seconds),
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
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(e) => {
            error!("Translation error: {}", e);
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}
