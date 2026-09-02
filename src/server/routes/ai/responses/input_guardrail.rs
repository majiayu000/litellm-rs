use crate::core::models::openai::continuation::map_responses_input_extensions;
use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::responses_api::ResponsesApiRequest;
use crate::core::models::openai::{ChatMessage, MessageContent};
use crate::core::providers::ChatMessageContinuation;
use crate::core::types::codex::domain::CodexTurnError;
use crate::server::routes::ai::chat::{
    extensions_after_input_projection, input_guardrail_request_with_extensions,
};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::{build_chat_request_from_turn, build_responses_continuation_turn};

pub(super) struct GuardedResponsesInput {
    pub(super) request: ResponsesApiRequest,
    pub(super) chat_request: ChatCompletionRequest,
    pub(super) chat_extensions: Vec<ChatMessageContinuation>,
}

pub(super) enum InputGuardrailError {
    Guardrail(GatewayError),
    Unsupported(String, String),
    Validation(String),
}

pub(super) fn validate_delivery(
    state: &AppState,
    request: &ResponsesApiRequest,
) -> Result<(), GatewayError> {
    if request.background.unwrap_or(false) && request.stream.unwrap_or(false) {
        return Err(GatewayError::validation(
            "background responses do not support stream=true",
        ));
    }
    if request.stream.unwrap_or(false) {
        crate::server::guardrails::reject_unsupported_streaming_mask(state)?;
    }
    Ok(())
}

pub(super) async fn apply(
    state: &AppState,
    request: ResponsesApiRequest,
    input_extensions: Vec<Option<ChatMessageContinuation>>,
    continuation_requested: bool,
) -> Result<GuardedResponsesInput, InputGuardrailError> {
    let (original_chat, original_extensions) =
        provider_projection(&request, input_extensions.clone())?;
    let continuation = continuation_requested
        .then(|| continuation_projection(&original_chat, &original_extensions))
        .transpose()
        .map_err(InputGuardrailError::Guardrail)?;
    let request = crate::server::guardrails::apply_responses_input(
        state.guardrails.as_ref(),
        &request,
        continuation.as_ref(),
    )
    .await
    .map_err(InputGuardrailError::Guardrail)?;
    let (chat_request, chat_extensions) = provider_projection(&request, input_extensions)?;
    let chat_extensions =
        extensions_after_input_projection(&original_chat, &chat_request, chat_extensions)
            .map_err(InputGuardrailError::Guardrail)?;
    Ok(GuardedResponsesInput {
        request,
        chat_request,
        chat_extensions,
    })
}

fn continuation_projection(
    request: &ChatCompletionRequest,
    extensions: &[ChatMessageContinuation],
) -> Result<ChatCompletionRequest, GatewayError> {
    let projection = ChatCompletionRequest {
        model: request.model.clone(),
        messages: request
            .messages
            .iter()
            .map(|message| ChatMessage {
                role: message.role.clone(),
                content: None::<MessageContent>,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            })
            .collect(),
        ..Default::default()
    };
    input_guardrail_request_with_extensions(&projection, extensions)
}

fn provider_projection(
    request: &ResponsesApiRequest,
    input_extensions: Vec<Option<ChatMessageContinuation>>,
) -> Result<(ChatCompletionRequest, Vec<ChatMessageContinuation>), InputGuardrailError> {
    let turn =
        build_responses_continuation_turn(request, &input_extensions).map_err(
            |error| match error {
                CodexTurnError::UnsupportedFeature(feature) => {
                    InputGuardrailError::Unsupported(feature, request.model.clone())
                }
                error => InputGuardrailError::Validation(error.to_string()),
            },
        )?;
    let chat_request =
        build_chat_request_from_turn(request, &turn).map_err(InputGuardrailError::Validation)?;
    let chat_extensions = map_responses_input_extensions(request, &chat_request, input_extensions)
        .map_err(InputGuardrailError::Validation)?;
    Ok((chat_request, chat_extensions))
}
