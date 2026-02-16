//! Text completions endpoint (legacy)
//!
//! This endpoint converts legacy text completions to chat completions format
//! since most modern providers only support chat completions.

use crate::core::models::openai::{CompletionRequest, CompletionResponse};
use crate::core::providers::ProviderRegistry;
use crate::core::types::{
    chat::ChatMessage, context::RequestContext, message::MessageContent, message::MessageRole,
    model::ProviderCapability,
};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::info;

use super::context::handle_ai_request;
use super::provider_selection::select_provider_for_model;

/// Text completions endpoint (legacy)
///
/// OpenAI-compatible text completions API for backward compatibility.
/// Internally converts to chat completions format.
pub async fn completions(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<CompletionRequest>,
) -> ActixResult<HttpResponse> {
    info!("Text completion request for model: {}", request.model);

    handle_ai_request(
        &req,
        request.into_inner(),
        "Text completion",
        |request, context| handle_completion_via_pool(&state.router, request, context),
    )
    .await
}

/// Handle completion via provider pool
///
/// Converts legacy text completion request to chat completion format
pub async fn handle_completion_via_pool(
    pool: &ProviderRegistry,
    request: CompletionRequest,
    context: RequestContext,
) -> Result<CompletionResponse, GatewayError> {
    if request.stream.unwrap_or(false) {
        return Err(GatewayError::validation(
            "Streaming text completions are not supported",
        ));
    }

    let selection =
        select_provider_for_model(pool, &request.model, ProviderCapability::ChatCompletion)?;

    let logit_bias = request.logit_bias.map(|bias| {
        bias.into_iter()
            .map(|(key, value)| (key, value as f32))
            .collect()
    });

    let options = crate::core::types::chat::ChatRequest {
        model: selection.model,
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text(request.prompt.clone())),
            thinking: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        }],
        temperature: request.temperature.map(|t| t as f32),
        max_tokens: request.max_tokens,
        top_p: request.top_p.map(|t| t as f32),
        frequency_penalty: request.frequency_penalty.map(|f| f as f32),
        presence_penalty: request.presence_penalty.map(|p| p as f32),
        stop: request.stop,
        stream: false,
        user: request.user,
        n: request.n,
        logprobs: request.logprobs.map(|_| true),
        top_logprobs: request.logprobs,
        logit_bias,
        ..Default::default()
    };

    let response = selection
        .provider
        .chat_completion(options, context)
        .await
        .map_err(|e| GatewayError::internal(format!("Completion error: {}", e)))?;

    // Convert chat completion response to text completion format
    let text = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .map(|c: &MessageContent| c.to_string())
        .unwrap_or_default();

    // Convert finish reason to string
    let finish_reason = response
        .choices
        .first()
        .and_then(|c| c.finish_reason.as_ref())
        .map(|fr| format!("{:?}", fr).to_lowercase());

    Ok(CompletionResponse {
        id: response.id,
        object: "text_completion".to_string(),
        created: response.created as u64,
        model: response.model,
        choices: vec![crate::core::models::openai::CompletionChoice {
            text,
            index: 0,
            logprobs: None,
            finish_reason,
        }],
        usage: response.usage.map(|u| crate::core::models::openai::Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        }),
    })
}
