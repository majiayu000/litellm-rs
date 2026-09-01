use crate::core::models::openai::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, ContentPart, MessageContent,
    MessageRole,
};
use crate::core::providers::ChatMessageContinuation;
use crate::utils::error::gateway_error::GatewayError;

pub(super) fn guardrail_response_with_continuation(
    response: &ChatCompletionResponse,
    choice_extensions: &[ChatMessageContinuation],
) -> Result<ChatCompletionResponse, GatewayError> {
    if response.choices.len() != choice_extensions.len() {
        return Err(GatewayError::internal(format!(
            "chat choice extensions length mismatch: expected {}, got {}",
            response.choices.len(),
            choice_extensions.len()
        )));
    }
    let mut projected = response.clone();
    for (choice, extension) in projected.choices.iter_mut().zip(choice_extensions) {
        append_guardrail_visible_thinking(&mut choice.message, extension);
    }
    Ok(projected)
}

pub(super) fn guardrail_request_with_continuation(
    request: &ChatCompletionRequest,
    message_extensions: &[ChatMessageContinuation],
) -> Result<ChatCompletionRequest, GatewayError> {
    if request.messages.len() != message_extensions.len() {
        return Err(GatewayError::validation(format!(
            "chat message extensions length mismatch: expected {}, got {}",
            request.messages.len(),
            message_extensions.len()
        )));
    }
    let mut projected = request.clone();
    for (message, extension) in projected.messages.iter_mut().zip(message_extensions) {
        extension.validate().map_err(GatewayError::validation)?;
        if !extension.is_empty() && message.role != MessageRole::Assistant {
            return Err(GatewayError::validation(
                "Anthropic continuation is only valid on assistant messages",
            ));
        }
        append_guardrail_visible_thinking(message, extension);
    }
    Ok(projected)
}

fn append_guardrail_visible_thinking(
    message: &mut ChatMessage,
    extension: &ChatMessageContinuation,
) {
    let Some(visible) = extension
        .anthropic_thinking()
        .and_then(|thinking| thinking.as_text())
    else {
        return;
    };
    match &mut message.content {
        Some(MessageContent::Text(content)) => {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(&visible);
        }
        Some(MessageContent::Parts(parts)) => parts.push(ContentPart::Text {
            text: visible.into_owned(),
        }),
        None => message.content = Some(MessageContent::Text(visible.into_owned())),
    }
}
