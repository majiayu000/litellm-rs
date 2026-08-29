//! OpenAI-compatible Responses API endpoint.

use crate::core::models::openai::continuation::{
    ResponsesApiRequestWithExtensions, ResponsesApiResponseWithExtensions,
    attach_responses_choice_extensions, build_responses_continuation_turn,
    map_responses_input_extensions,
};
use crate::core::models::openai::messages::{
    ChatMessage, ContentPart, ImageUrl, MessageContent, MessageRole,
};
use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::responses_api::{
    ResponseFunctionCall, ResponseInput, ResponseInputContent, ResponseInputContentPart,
    ResponseInputItem, ResponseOutputContent, ResponseOutputItem, ResponseOutputMessage,
    ResponseTool, ResponseUsage, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::models::openai::tools::{Function, FunctionCall, Tool, ToolCall};
use crate::core::types::anthropic_continuation::ChatMessageExtensions;
use crate::core::types::codex::domain::{CodexTurn, CodexTurnError, CodexTurnItem};
use crate::core::types::codex::wire::{CodexCustomToolCall, CodexToolOutput};
use crate::core::types::responses::FinishReason;
use crate::server::routes::ai::chat::{
    handle_chat_completion_with_extensions, handle_chat_completion_with_state,
};
use crate::server::state::AppState;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::{error, info};

use super::openai_errors;
#[cfg(test)]
#[path = "responses/codex_compat_tests.rs"]
mod codex_compat_tests;
mod lifecycle;
pub(crate) use lifecycle::{ResponseOwner, store_response_if_requested};
pub use lifecycle::{cancel_response, delete_response, get_response, list_response_input_items};
#[cfg(test)]
static PROVIDER_DISPATCH_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static CODEX_DISPATCH_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn create_response(
    state: web::Data<AppState>,
    req: HttpRequest,
    payload: web::Json<ResponsesApiRequestWithExtensions>,
) -> ActixResult<HttpResponse> {
    info!(
        "Responses API request for model: {}",
        payload.legacy().model
    );

    let context = match super::token_policy::shared_request_context_with_api_key_token_limit(&req) {
        Ok(context) => context,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };
    let owner = lifecycle::response_owner(context.as_ref());
    let typed_request = payload.into_inner();
    let continuation_requested =
        match super::chat::continuation_opt_in(&req, typed_request.has_continuation()) {
            Ok(opt_in) => opt_in,
            Err(error) => return Ok(openai_errors::validation_error(error)),
        };
    let (request, input_extensions) = typed_request.into_parts();

    if continuation_requested
        && (request.stream.unwrap_or(false)
            || request.background.unwrap_or(false)
            || request.store.unwrap_or(true)
            || request.previous_response_id.is_some())
    {
        return Ok(openai_errors::validation_error(
            "Anthropic continuation does not yet support stream, background, store, or previous_response_id",
        ));
    }

    if request.model.trim().is_empty() {
        return Ok(openai_errors::validation_error("model must not be empty"));
    }
    if let Err(error) = super::context::enforce_api_key_model_and_token_limits(
        &req,
        &request.model,
        request.max_output_tokens,
    ) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    match &request.input {
        ResponseInput::Text(t) if t.trim().is_empty() => {
            return Ok(openai_errors::validation_error(
                "input text must not be empty",
            ));
        }
        ResponseInput::Items(items) if items.is_empty() => {
            return Ok(openai_errors::validation_error(
                "input array must not be empty",
            ));
        }
        _ => {}
    }

    if let Err(error) = lifecycle::validate_storage_owner(&request, &owner) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let (request, input_extensions) =
        match lifecycle::resolve_previous_response_context_with_extensions(
            request,
            input_extensions,
            &owner,
        ) {
            Ok(resolved) => resolved,
            Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
        };

    let turn = match build_responses_continuation_turn(&request, &input_extensions) {
        Ok(turn) => turn,
        Err(CodexTurnError::UnsupportedFeature(feature)) => {
            return Ok(openai_errors::unsupported_codex_feature(
                &feature,
                &request.model,
            ));
        }
        Err(error) => return Ok(openai_errors::validation_error(error.to_string())),
    };
    let chat_request = match build_chat_request_from_turn(&request, &turn) {
        Ok(r) => r,
        Err(e) => return Ok(openai_errors::validation_error(e)),
    };
    let chat_extensions =
        match map_responses_input_extensions(&request, &chat_request, input_extensions) {
            Ok(extensions) => extensions,
            Err(error) => return Ok(openai_errors::validation_error(error)),
        };
    #[cfg(test)]
    if req.headers().contains_key("x-codex-upstream-counter") {
        PROVIDER_DISPATCH_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    if request.background.unwrap_or(false) {
        if request.stream.unwrap_or(false) {
            return Ok(openai_errors::validation_error(
                "background responses do not support stream=true",
            ));
        }
        Ok(lifecycle::handle_background_response(
            state.get_ref().clone(),
            chat_request,
            request,
            context.as_ref().clone(),
            owner,
        ))
    } else if request.stream.unwrap_or(false) {
        super::responses_stream::handle_streaming_response(
            state.get_ref(),
            chat_request,
            request,
            context,
            owner,
        )
        .await
    } else {
        handle_sync_response(
            state.get_ref(),
            chat_request,
            chat_extensions,
            continuation_requested,
            request,
            context.as_ref().clone(),
            owner,
        )
        .await
    }
}

async fn handle_sync_response(
    state: &AppState,
    chat_request: ChatCompletionRequest,
    chat_extensions: Vec<ChatMessageExtensions>,
    has_continuation: bool,
    original: ResponsesApiRequest,
    mut context: crate::core::types::context::RequestContext,
    owner: Option<lifecycle::ResponseOwner>,
) -> ActixResult<HttpResponse> {
    super::response_cache::bypass_chat_response_cache(&mut context);
    let result = if has_continuation {
        handle_chat_completion_with_extensions(
            state,
            std::sync::Arc::new(chat_request),
            std::sync::Arc::new(context),
            chat_extensions,
            true,
        )
        .await
        .map(|response| response.into_parts())
    } else {
        handle_chat_completion_with_state(state, chat_request, context)
            .await
            .map(|response| (response, Vec::new()))
    };
    match result {
        Ok((chat_resp, choice_extensions)) => {
            let mut resp = convert_to_responses_api(chat_resp, &original);
            let output_extensions = match attach_responses_choice_extensions(
                &mut resp,
                choice_extensions,
                empty_output_message(),
            ) {
                Ok(extensions) => extensions,
                Err(error) => return Ok(openai_errors::validation_error(error)),
            };
            lifecycle::store_response_if_requested(&original, &resp, owner);
            match ResponsesApiResponseWithExtensions::from_parts(resp, output_extensions) {
                Ok(response) => Ok(HttpResponse::Ok().json(response)),
                Err(error) => Ok(openai_errors::validation_error(error)),
            }
        }
        Err(e) => {
            error!("Responses API error: {}", e);
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}

fn empty_output_message() -> ResponseOutputItem {
    ResponseOutputItem::Message(ResponseOutputMessage {
        id: format!("msg_{}", uuid_v4_hex()),
        role: "assistant".to_string(),
        status: "completed".to_string(),
        content: vec![],
    })
}

#[cfg(test)]
pub(crate) fn build_chat_request(
    req: &ResponsesApiRequest,
) -> Result<ChatCompletionRequest, String> {
    let turn = CodexTurn::try_from(req).map_err(|error| error.to_string())?;
    build_chat_request_from_turn(req, &turn)
}

fn build_chat_request_from_turn(
    req: &ResponsesApiRequest,
    turn: &CodexTurn,
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

    for item in &turn.items {
        let message = match item {
            CodexTurnItem::Text(text) => ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text(text.clone())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
            CodexTurnItem::Item(ResponseInputItem::Message(message)) => {
                response_message_to_chat(message)?
            }
            CodexTurnItem::Item(ResponseInputItem::FunctionCall(call)) => {
                push_tool_call(
                    &mut messages,
                    &call.call_id,
                    &call.name,
                    call.arguments.clone(),
                );
                continue;
            }
            CodexTurnItem::Item(ResponseInputItem::CustomToolCall(call)) => {
                push_tool_call(
                    &mut messages,
                    &call.call_id,
                    &call.name,
                    serde_json::json!({"input": call.input}).to_string(),
                );
                continue;
            }
            CodexTurnItem::Item(ResponseInputItem::FunctionCallOutput(output)) => {
                tool_output_message(&output.call_id, tool_output_text(&output.output)?)
            }
            CodexTurnItem::Item(ResponseInputItem::CustomToolCallOutput(output)) => {
                tool_output_message(&output.call_id, tool_output_text(&output.output)?)
            }
            CodexTurnItem::Item(item) => {
                return Err(format!(
                    "unsupported Codex feature: {}",
                    item.feature_name()
                ));
            }
        };
        messages.push(message);
    }

    if messages.is_empty() {
        return Err("input must contain at least one message".to_string());
    }

    let mut tools = Vec::new();
    for tool in &turn.tools {
        let function = match tool {
            ResponseTool::Function(tool) => Function {
                name: tool.function.name.clone(),
                description: tool.function.description.clone(),
                parameters: tool.function.parameters.clone(),
            },
            ResponseTool::CodexFunction(tool) => Function {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
            ResponseTool::Custom(tool) => Function {
                name: tool.name.clone(),
                description: Some(tool.description.clone()),
                parameters: Some(serde_json::json!({
                    "type": "object",
                    "properties": {"input": {"type": "string"}},
                    "required": ["input"],
                    "additionalProperties": false
                })),
            },
            tool => {
                return Err(format!(
                    "unsupported Codex feature: {}",
                    tool.feature_name()
                ));
            }
        };
        tools.push(Tool {
            tool_type: "function".to_string(),
            function,
        });
    }

    Ok(ChatCompletionRequest {
        model: req.model.clone(),
        messages,
        temperature: req.temperature,
        max_tokens: req.max_output_tokens,
        max_completion_tokens: req.max_output_tokens,
        top_p: req.top_p,
        stream: req.stream,
        user: req.user.clone(),
        tools: if tools.is_empty() { None } else { Some(tools) },
        reasoning_effort: req.reasoning.as_ref().and_then(|r| r.effort.clone()),
        store: req.store,
        metadata: req.metadata.clone(),
        ..Default::default()
    })
}

fn response_message_to_chat(
    message: &crate::core::models::openai::responses_api::ResponseInputMessage,
) -> Result<ChatMessage, String> {
    let content = match &message.content {
        ResponseInputContent::Text(text) => Some(MessageContent::Text(text.clone())),
        ResponseInputContent::Parts(parts) => {
            let mut converted = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    ResponseInputContentPart::InputText { text }
                    | ResponseInputContentPart::OutputText { text } => {
                        converted.push(ContentPart::Text { text: text.clone() });
                    }
                    ResponseInputContentPart::InputImage { image_url, detail } => {
                        let url = image_url
                            .as_ref()
                            .ok_or_else(|| "input_image part is missing image_url".to_string())?;
                        converted.push(ContentPart::ImageUrl {
                            image_url: ImageUrl {
                                url: url.clone(),
                                detail: detail.clone(),
                            },
                        });
                    }
                    ResponseInputContentPart::InputAudio { .. } => {
                        return Err("unsupported Codex feature: input_audio".to_string());
                    }
                }
            }
            match converted.as_slice() {
                [] => None,
                [ContentPart::Text { text }] => Some(MessageContent::Text(text.clone())),
                _ => Some(MessageContent::Parts(converted)),
            }
        }
    };
    Ok(ChatMessage {
        role: parse_role(&message.role)?,
        content,
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    })
}

fn push_tool_call(messages: &mut Vec<ChatMessage>, call_id: &str, name: &str, arguments: String) {
    let tool_call = ToolCall {
        id: call_id.to_string(),
        tool_type: "function".to_string(),
        function: FunctionCall {
            name: name.to_string(),
            arguments,
        },
    };
    if let Some(message) = messages.last_mut()
        && message.role == MessageRole::Assistant
    {
        message.tool_calls.get_or_insert_default().push(tool_call);
        return;
    }
    messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        name: None,
        function_call: None,
        tool_calls: Some(vec![tool_call]),
        tool_call_id: None,
        audio: None,
    });
}

fn tool_output_message(call_id: &str, output: String) -> ChatMessage {
    ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text(output)),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: Some(call_id.to_string()),
        audio: None,
    }
}

fn tool_output_text(output: &CodexToolOutput) -> Result<String, String> {
    match output {
        CodexToolOutput::Text(text) => Ok(text.clone()),
        CodexToolOutput::ContentItems(items) => {
            serde_json::to_string(items).map_err(|error| format!("invalid tool output: {error}"))
        }
    }
}

pub(crate) fn convert_to_responses_api(
    chat: crate::core::models::openai::responses::ChatCompletionResponse,
    original: &ResponsesApiRequest,
) -> ResponsesApiResponse {
    let resp_id = format!("resp_{}", &chat.id);

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

            let text_content: Vec<ResponseOutputContent> = match &choice.message.content {
                Some(MessageContent::Text(t)) if !t.is_empty() => {
                    vec![ResponseOutputContent::OutputText {
                        text: t.clone(),
                        annotations: None,
                        logprobs: None,
                    }]
                }
                Some(MessageContent::Parts(parts)) => parts
                    .iter()
                    .filter_map(|part| {
                        if let ContentPart::Text { text } = part
                            && !text.is_empty()
                        {
                            return Some(ResponseOutputContent::OutputText {
                                text: text.clone(),
                                annotations: None,
                                logprobs: None,
                            });
                        }
                        None
                    })
                    .collect(),
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

            if let Some(tool_calls) = choice.message.tool_calls {
                for tc in tool_calls {
                    if is_custom_tool(original, &tc.function.name) {
                        items.push(ResponseOutputItem::CustomToolCall(CodexCustomToolCall {
                            id: Some(format!("ct_{}", uuid_v4_hex())),
                            call_id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            namespace: None,
                            input: custom_tool_input(&tc.function.arguments),
                            status: Some(finish_status.to_string()),
                            internal_chat_message_metadata_passthrough: None,
                        }));
                    } else {
                        items.push(ResponseOutputItem::FunctionCall(ResponseFunctionCall {
                            id: format!("fc_{}", uuid_v4_hex()),
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                            status: finish_status.to_string(),
                            call_id: Some(tc.id.clone()),
                        }));
                    }
                }
            }

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

pub(crate) fn is_custom_tool(original: &ResponsesApiRequest, name: &str) -> bool {
    original.tools.as_ref().is_some_and(|tools| {
        tools
            .iter()
            .any(|tool| matches!(tool, ResponseTool::Custom(tool) if tool.name == name))
    })
}

pub(crate) fn custom_tool_input(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|value| {
            value
                .get("input")
                .and_then(|input| input.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| arguments.to_string())
}

pub(crate) fn finish_reason_to_status(reason: Option<&str>) -> &'static str {
    match reason {
        Some("length") => "incomplete",
        Some("content_filter") => "failed",
        _ => "completed",
    }
}

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
        "developer" => Ok(MessageRole::Developer),
        other => Err(format!("unknown message role: {other}")),
    }
}

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

pub(crate) fn current_unix_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::responses_api::{ResponseInput, ResponsesApiRequest};
    use crate::core::types::anthropic_continuation::{
        AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    };

    fn minimal_request(input: &str) -> ResponsesApiRequest {
        ResponsesApiRequest {
            model: "gpt-4o".to_string(),
            input: ResponseInput::Text(input.to_string()),
            instructions: None,
            previous_response_id: None,
            store: None,
            tools: None,
            additional_tools: None,
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
        assert_eq!(chat.max_tokens, Some(512));
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
    fn continuation_message_stays_attached_to_following_tool_call_and_result() {
        let request: ResponsesApiRequest = serde_json::from_value(serde_json::json!({
            "model":"claude-opus-5", "input":[
                {"type":"message","role":"assistant","content":[{"type":"input_text","text":"visible answer"}]},
                {"type":"function_call","id":"fc_1","call_id":"toolu_1","name":"lookup","arguments":"{}"},
                {"type":"custom_tool_call","id":"ct_1","call_id":"toolu_2","name":"shell","input":"pwd"},
                {"type":"function_call_output","call_id":"toolu_1","output":"result"},
                {"type":"custom_tool_call_output","call_id":"toolu_2","name":"shell","output":"/tmp"}]
        })).unwrap();
        let extension = ChatMessageExtensions::new().with_anthropic_thinking(
            AnthropicThinkingContent::new(vec![AnthropicThinkingBlock::Thinking {
                thinking: "plan".into(),
                signature: AnthropicSignature::try_from("opaque-signature").unwrap(),
            }]),
        );
        let input = vec![Some(extension), None, None, None, None];
        let turn = build_responses_continuation_turn(&request, &input).unwrap();
        let chat = build_chat_request_from_turn(&request, &turn).unwrap();
        let mapped = map_responses_input_extensions(&request, &chat, input).unwrap();
        assert_eq!((chat.messages.len(), mapped.len()), (3, 3));
        assert!(!mapped[0].is_empty());
        assert!(mapped[1].is_empty());
        assert!(mapped[2].is_empty());
        assert_eq!(
            chat.messages[0].tool_calls.as_ref().unwrap()[0].id,
            "toolu_1"
        );
        assert_eq!(
            chat.messages[0].tool_calls.as_ref().unwrap()[1].id,
            "toolu_2"
        );
        assert!(matches!(
            chat.messages[0].content.as_ref(),
            Some(MessageContent::Text(text)) if text == "visible answer"
        ));
        assert_eq!(chat.messages[1].tool_call_id.as_deref(), Some("toolu_1"));
        assert!(matches!(
            chat.messages[1].content.as_ref(),
            Some(MessageContent::Text(text)) if text == "result"
        ));
        assert_eq!(chat.messages[2].tool_call_id.as_deref(), Some("toolu_2"));
        assert!(matches!(
            chat.messages[2].content.as_ref(),
            Some(MessageContent::Text(text)) if text == "/tmp"
        ));
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
