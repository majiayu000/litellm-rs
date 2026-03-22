//! POST /v1/responses — OpenAI Responses API endpoint
//!
//! Accepts Responses API requests, converts to internal chat-completion format,
//! forwards to the selected provider, and converts results back.

use crate::core::models::openai::messages::{
    ChatMessage, ContentPart, ImageUrl, MessageContent, MessageRole,
};
use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::responses_api::{
    ResponseFunctionCall, ResponseInput, ResponseInputContent, ResponseInputContentPart,
    ResponseInputItem, ResponseOutputContent, ResponseOutputItem, ResponseOutputMessage,
    ResponseTool, ResponseUsage, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::types::responses::FinishReason;
use crate::server::routes::ai::chat::handle_chat_completion_with_state;
use crate::server::routes::errors;
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, ResponseError, Result as ActixResult, web};
use tracing::{error, info};

/// POST /v1/responses handler
pub async fn create_response(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<ResponsesApiRequest>,
) -> ActixResult<HttpResponse> {
    info!("Responses API request for model: {}", body.model);

    let context = super::context::get_request_context(&req)?;
    let request = body.into_inner();

    if request.model.trim().is_empty() {
        return Ok(errors::validation_error("model must not be empty"));
    }

    match &request.input {
        ResponseInput::Text(t) if t.trim().is_empty() => {
            return Ok(errors::validation_error("input text must not be empty"));
        }
        ResponseInput::Items(items) if items.is_empty() => {
            return Ok(errors::validation_error("input array must not be empty"));
        }
        _ => {}
    }

    let chat_request = match build_chat_request(&request) {
        Ok(r) => r,
        Err(e) => return Ok(errors::validation_error(&e)),
    };

    if request.stream.unwrap_or(false) {
        super::responses_stream::handle_streaming_response(
            state.get_ref(),
            chat_request,
            request,
            context,
        )
        .await
    } else {
        handle_sync_response(state.get_ref(), chat_request, request, context).await
    }
}

// ── Non-streaming path ────────────────────────────────────────────────────────

async fn handle_sync_response(
    state: &AppState,
    chat_request: ChatCompletionRequest,
    original: ResponsesApiRequest,
    context: crate::core::types::context::RequestContext,
) -> ActixResult<HttpResponse> {
    match handle_chat_completion_with_state(state, chat_request, context).await {
        Ok(chat_resp) => {
            let resp = convert_to_responses_api(chat_resp, &original);
            Ok(HttpResponse::Ok().json(resp))
        }
        Err(e) => {
            error!("Responses API error: {}", e);
            Ok(e.error_response())
        }
    }
}

// ── Request conversion ────────────────────────────────────────────────────────

/// Convert a `ResponsesApiRequest` to a `ChatCompletionRequest`.
pub(crate) fn build_chat_request(
    req: &ResponsesApiRequest,
) -> Result<ChatCompletionRequest, String> {
    let mut messages: Vec<ChatMessage> = Vec::new();

    if let Some(instructions) = &req.instructions {
        messages.push(ChatMessage {
            role: MessageRole::System,
            content: Some(MessageContent::Text(instructions.to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        });
    }

    match &req.input {
        ResponseInput::Text(text) => {
            messages.push(ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text(text.clone())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            });
        }
        ResponseInput::Items(items) => {
            for item in items {
                let ResponseInputItem::Message(msg) = item;
                let role = parse_role(&msg.role)?;
                let content = match &msg.content {
                    ResponseInputContent::Text(t) => MessageContent::Text(t.clone()),
                    ResponseInputContent::Parts(parts) => {
                        let content_parts: Vec<ContentPart> = parts
                            .iter()
                            .filter_map(|p| match p {
                                ResponseInputContentPart::InputText { text }
                                | ResponseInputContentPart::OutputText { text } => {
                                    Some(ContentPart::Text { text: text.clone() })
                                }
                                ResponseInputContentPart::InputImage { image_url, detail } => {
                                    image_url.as_ref().map(|url| ContentPart::ImageUrl {
                                        image_url: ImageUrl {
                                            url: url.clone(),
                                            detail: detail.clone(),
                                        },
                                    })
                                }
                            })
                            .collect();
                        if content_parts.len() == 1 {
                            if let ContentPart::Text { text } = &content_parts[0] {
                                MessageContent::Text(text.clone())
                            } else {
                                MessageContent::Parts(content_parts)
                            }
                        } else {
                            MessageContent::Parts(content_parts)
                        }
                    }
                };
                messages.push(ChatMessage {
                    role,
                    content: Some(content),
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                    audio: None,
                });
            }
        }
    }

    if messages.is_empty() {
        return Err("input must contain at least one message".to_string());
    }

    let mut tools: Vec<crate::core::models::openai::tools::Tool> = Vec::new();
    if let Some(req_tools) = &req.tools {
        for t in req_tools {
            match t {
                ResponseTool::Function(f) => {
                    tools.push(crate::core::models::openai::tools::Tool {
                        tool_type: "function".to_string(),
                        function: crate::core::models::openai::tools::Function {
                            name: f.function.name.clone(),
                            description: f.function.description.clone(),
                            parameters: f.function.parameters.clone(),
                        },
                    });
                }
                ResponseTool::WebSearch(_)
                | ResponseTool::WebSearchPreview(_)
                | ResponseTool::FileSearch(_)
                | ResponseTool::CodeInterpreter(_)
                | ResponseTool::ComputerUsePreview(_)
                | ResponseTool::Mcp(_) => {
                    return Err(
                        "built-in tools (web_search, file_search, code_interpreter, mcp, \
                         computer_use) are not supported via the chat-completions proxy path"
                            .to_string(),
                    );
                }
            }
        }
    }

    Ok(ChatCompletionRequest {
        model: req.model.clone(),
        messages,
        temperature: req.temperature,
        max_completion_tokens: req.max_output_tokens,
        top_p: req.top_p,
        stream: req.stream,
        user: req.user.clone(),
        tools: if tools.is_empty() { None } else { Some(tools) },
        reasoning_effort: req.reasoning.as_ref().and_then(|r| r.effort.clone()),
        ..Default::default()
    })
}

// ── Response conversion ───────────────────────────────────────────────────────

/// Convert a `ChatCompletionResponse` to a `ResponsesApiResponse`.
pub(crate) fn convert_to_responses_api(
    chat: crate::core::models::openai::responses::ChatCompletionResponse,
    original: &ResponsesApiRequest,
) -> ResponsesApiResponse {
    let resp_id = format!("resp_{}", &chat.id);

    // Determine overall status from the first choice's finish_reason.
    let overall_status = chat
        .choices
        .first()
        .and_then(|c| c.finish_reason.as_deref())
        .map(|r| finish_reason_to_status(Some(r)))
        .unwrap_or("completed");

    let output: Vec<ResponseOutputItem> = chat
        .choices
        .into_iter()
        .flat_map(|choice| {
            let finish_status = finish_reason_to_status(choice.finish_reason.as_deref());
            let mut items: Vec<ResponseOutputItem> = Vec::new();

            // Text content → message output item
            let text_content: Vec<ResponseOutputContent> = match &choice.message.content {
                Some(MessageContent::Text(t)) if !t.is_empty() => {
                    vec![ResponseOutputContent::OutputText {
                        text: t.clone(),
                        annotations: None,
                        logprobs: None,
                    }]
                }
                _ => vec![],
            };
            if !text_content.is_empty() {
                items.push(ResponseOutputItem::Message(ResponseOutputMessage {
                    id: format!("msg_{}", uuid_v4_hex()),
                    role: "assistant".to_string(),
                    status: finish_status.to_string(),
                    content: text_content,
                }));
            }

            // Tool calls → function call output items
            if let Some(tool_calls) = choice.message.tool_calls {
                for tc in tool_calls {
                    items.push(ResponseOutputItem::FunctionCall(ResponseFunctionCall {
                        id: format!("fc_{}", uuid_v4_hex()),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                        status: finish_status.to_string(),
                        call_id: Some(tc.id.clone()),
                    }));
                }
            }

            // Ensure at least one output item per choice
            if items.is_empty() {
                items.push(ResponseOutputItem::Message(ResponseOutputMessage {
                    id: format!("msg_{}", uuid_v4_hex()),
                    role: "assistant".to_string(),
                    status: finish_status.to_string(),
                    content: vec![],
                }));
            }

            items
        })
        .collect();

    let usage = chat.usage.map(|u| ResponseUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
        input_tokens_details: u.prompt_tokens_details.map(|d| {
            crate::core::models::openai::responses_api::ResponseInputTokensDetails {
                cached_tokens: d.cached_tokens.unwrap_or(0),
            }
        }),
        output_tokens_details: u.completion_tokens_details.map(|d| {
            crate::core::models::openai::responses_api::ResponseOutputTokensDetails {
                reasoning_tokens: d.reasoning_tokens.unwrap_or(0),
            }
        }),
    });

    ResponsesApiResponse {
        id: resp_id,
        object: "response".to_string(),
        created_at: chat.created as i64,
        status: overall_status.to_string(),
        model: chat.model,
        output,
        usage,
        error: None,
        previous_response_id: original.previous_response_id.clone(),
        metadata: original.metadata.clone(),
    }
}

/// Map a chat-completion `finish_reason` to a Responses API item/response status.
///
/// - `"stop"` / `"tool_calls"` / `None` → `"completed"` (normal completion)
/// - `"length"` → `"incomplete"` (truncated by token limit)
/// - `"content_filter"` → `"failed"` (safety filter triggered)
pub(crate) fn finish_reason_to_status(reason: Option<&str>) -> &'static str {
    match reason {
        Some("length") => "incomplete",
        Some("content_filter") => "failed",
        _ => "completed",
    }
}

/// Same mapping as [`finish_reason_to_status`] but for the typed `FinishReason` enum
/// used by the internal streaming types.
pub(crate) fn finish_reason_enum_to_status(reason: Option<&FinishReason>) -> &'static str {
    match reason {
        Some(FinishReason::Length) => "incomplete",
        Some(FinishReason::ContentFilter) => "failed",
        _ => "completed",
    }
}

pub(crate) fn parse_role(role: &str) -> Result<MessageRole, String> {
    match role {
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "system" => Ok(MessageRole::System),
        other => Err(format!("unknown message role: {other}")),
    }
}

/// Generate a collision-resistant hex identifier for response/message IDs.
///
/// Combines full nanoseconds since epoch with a process-global atomic counter
/// so IDs remain unique across concurrent calls within the same second.
pub(crate) fn uuid_v4_hex() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:016x}{seq:08x}")
}

/// Current Unix timestamp in seconds.
pub(crate) fn current_unix_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::responses_api::{ResponseInput, ResponsesApiRequest};

    fn minimal_request(input: &str) -> ResponsesApiRequest {
        ResponsesApiRequest {
            model: "gpt-4o".to_string(),
            input: ResponseInput::Text(input.to_string()),
            instructions: None,
            previous_response_id: None,
            store: None,
            tools: None,
            stream: None,
            background: None,
            max_output_tokens: None,
            temperature: None,
            top_p: None,
            user: None,
            reasoning: None,
            metadata: None,
            truncation: None,
        }
    }

    #[test]
    fn test_text_input_becomes_user_message() {
        let req = minimal_request("Hello");
        let chat = build_chat_request(&req).unwrap();
        assert_eq!(chat.model, "gpt-4o");
        assert_eq!(chat.messages.len(), 1);
        assert!(matches!(chat.messages[0].role, MessageRole::User));
    }

    #[test]
    fn test_instructions_prepended_as_system() {
        let mut req = minimal_request("Hi");
        req.instructions = Some("Be brief".to_string());
        let chat = build_chat_request(&req).unwrap();
        assert_eq!(chat.messages.len(), 2);
        assert!(matches!(chat.messages[0].role, MessageRole::System));
    }

    #[test]
    fn test_temperature_forwarded() {
        let mut req = minimal_request("test");
        req.temperature = Some(0.5);
        let chat = build_chat_request(&req).unwrap();
        assert_eq!(chat.temperature, Some(0.5));
    }

    #[test]
    fn test_max_output_tokens_maps_to_max_completion_tokens() {
        let mut req = minimal_request("test");
        req.max_output_tokens = Some(512);
        let chat = build_chat_request(&req).unwrap();
        assert_eq!(chat.max_completion_tokens, Some(512));
    }

    #[test]
    fn test_reasoning_effort_forwarded() {
        let mut req = minimal_request("test");
        req.reasoning = Some(
            crate::core::models::openai::responses_api::ReasoningParams {
                effort: Some("high".to_string()),
                summary: None,
            },
        );
        let chat = build_chat_request(&req).unwrap();
        assert_eq!(chat.reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    fn test_parse_role_valid_values() {
        assert!(matches!(parse_role("user").unwrap(), MessageRole::User));
        assert!(matches!(
            parse_role("assistant").unwrap(),
            MessageRole::Assistant
        ));
        assert!(matches!(parse_role("system").unwrap(), MessageRole::System));
    }

    #[test]
    fn test_parse_role_invalid_returns_error() {
        assert!(parse_role("unknown").is_err());
    }
}
