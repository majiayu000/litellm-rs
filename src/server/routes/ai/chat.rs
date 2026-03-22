//! Chat completions endpoint

use crate::core::models::openai::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ContentLogprob, Logprobs, Tool,
    ToolChoice, TopLogprob, Usage,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::{
    ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionDelta, Event,
};
use crate::core::types::{
    self, chat::ChatRequest as CoreChatRequest, context::RequestContext, model::ProviderCapability,
};
use crate::server::routes::errors;
use crate::server::state::AppState;
use crate::utils::data::validation::RequestValidator;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpRequest, HttpResponse, ResponseError, Result as ActixResult, web};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::context::get_request_context;
use super::provider_selection::select_provider_for_model;

/// Chat completions endpoint
///
/// OpenAI-compatible chat completions API that supports streaming and non-streaming responses.
pub async fn chat_completions(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ChatCompletionRequest>,
) -> ActixResult<HttpResponse> {
    info!("Chat completion request for model: {}", request.model);

    // Get request context from middleware
    let context = get_request_context(&req)?;

    // Validate request
    if let Err(e) = RequestValidator::validate_chat_completion_request(
        &request.model,
        &request.messages,
        request.max_tokens,
        request.temperature,
    ) {
        warn!("Invalid chat completion request: {}", e);
        return Ok(errors::validation_error(&e.to_string()));
    }

    // Check if streaming is requested
    if request.stream.unwrap_or(false) {
        // Handle streaming request
        handle_streaming_chat_completion(state.get_ref(), request.into_inner(), context).await
    } else {
        // Handle non-streaming request
        match handle_chat_completion_with_state(state.get_ref(), request.into_inner(), context)
            .await
        {
            Ok(response) => Ok(HttpResponse::Ok().json(response)),
            Err(e) => {
                error!("Chat completion error: {}", e);
                Ok(e.error_response())
            }
        }
    }
}

/// Handle streaming chat completion
async fn handle_streaming_chat_completion(
    state: &AppState,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> ActixResult<HttpResponse> {
    info!(
        "Handling streaming chat completion for model: {}",
        request.model
    );

    let unified_router = &state.unified_router;

    // Keep provider selection for capability validation before execution.
    if let Err(e) = select_provider_for_model(
        unified_router,
        &request.model,
        ProviderCapability::ChatCompletionStream,
    ) {
        return Ok(e.error_response());
    }

    let requested_model = request.model.clone();
    let core_request = match build_core_chat_request(request, requested_model, true) {
        Ok(req) => req,
        Err(e) => return Ok(e.error_response()),
    };

    let requested_model = core_request.model.clone();
    let context_for_execution = context.clone();
    match unified_router
        .execute_with_retry(&requested_model, move |deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            async move {
                let deployment =
                    unified_router
                        .get_deployment(&deployment_id)
                        .ok_or_else(|| {
                            ProviderError::other("router", "Selected deployment not found")
                        })?;

                let provider = deployment.provider.clone();
                let selected_model = deployment.model.clone();
                drop(deployment);

                let mut request_for_provider = core_request.clone();
                request_for_provider.model = selected_model;
                let stream = provider
                    .chat_completion_stream(request_for_provider, context)
                    .await?;
                Ok((stream, 0))
            }
        })
        .await
    {
        Ok((mut stream, _, _, _)) => {
            // Use a channel so that when the client disconnects and actix-web
            // drops the response stream, the receiver is dropped, which causes
            // the sender to fail. This breaks the upstream read loop and drops
            // the provider stream, cancelling any in-flight HTTP request.
            let (tx, rx) = mpsc::channel::<Bytes>(8);

            let idle_timeout_secs = state.config.load().gateway.server.stream_idle_timeout;

            tokio::spawn(async move {
                loop {
                    let chunk_result = if idle_timeout_secs == 0 {
                        stream.next().await
                    } else {
                        let timeout_dur = Duration::from_secs(idle_timeout_secs);
                        match tokio::time::timeout(timeout_dur, stream.next()).await {
                            Ok(result) => result,
                            Err(_) => {
                                warn!(
                                    "SSE stream idle timeout after {}s, closing connection",
                                    idle_timeout_secs
                                );
                                let error_bytes = format_sse_error(
                                    &format!(
                                        "Stream idle timeout: no data received for {}s",
                                        idle_timeout_secs
                                    ),
                                    "server_error",
                                    "timeout",
                                );
                                if tx.send(error_bytes).await.is_err() {
                                    info!("Client disconnected before timeout error could be sent");
                                }
                                break;
                            }
                        }
                    };

                    let Some(chunk_result) = chunk_result else {
                        break;
                    };

                    let bytes = match chunk_result {
                        Ok(chunk) => {
                            let chat_chunk = convert_core_chunk_to_streaming(chunk);
                            match serde_json::to_string(&chat_chunk) {
                                Ok(json) => {
                                    let event = Event::default().data(&json);
                                    event.to_bytes()
                                }
                                Err(e) => {
                                    error!("Stream serialization error: {}", e);
                                    let error_bytes = format_sse_error(
                                        &format!("Serialization error: {}", e),
                                        "server_error",
                                        "internal_error",
                                    );
                                    // Send error then stop reading upstream.
                                    if tx.send(error_bytes).await.is_err() {
                                        info!(
                                            "Client disconnected before error event could be sent"
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Stream chunk error: {}", e);
                            let (error_type, error_code) = sse_error_classification(&e);
                            let error_bytes =
                                format_sse_error(&e.to_string(), error_type, error_code);
                            if tx.send(error_bytes).await.is_err() {
                                info!("Client disconnected before error event could be sent");
                            }
                            break;
                        }
                    };

                    // If the receiver has been dropped (client disconnected),
                    // tx.send() returns Err and we exit, dropping the provider
                    // stream to cancel the upstream request.
                    if tx.send(bytes).await.is_err() {
                        info!("Client disconnected during streaming, cancelling upstream");
                        break;
                    }
                }

                // Send the final [DONE] event; ignore error if client is gone.
                let done_event = Event::default().data("[DONE]");
                if tx.send(done_event.to_bytes()).await.is_err() {
                    info!("Client disconnected before [DONE] event could be sent");
                }
                // `stream` (the provider stream) is dropped here, cancelling
                // any remaining upstream work.
            });

            let sse_stream = tokio_stream::wrappers::ReceiverStream::new(rx)
                .map(Ok::<_, actix_web::error::Error>);

            Ok(HttpResponse::Ok()
                .insert_header((CONTENT_TYPE, "text/event-stream"))
                .insert_header((CACHE_CONTROL, "no-cache"))
                .insert_header(("Connection", "keep-alive"))
                .insert_header(("X-Request-ID", context.request_id.as_str()))
                .streaming(sse_stream))
        }
        Err((e, _)) => {
            error!("Failed to create streaming response: {}", e);
            Ok(GatewayError::Provider(e).error_response())
        }
    }
}

/// Handle chat completion with app state (UnifiedRouter only)
pub async fn handle_chat_completion_with_state(
    state: &AppState,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    let unified_router = &state.unified_router;
    handle_chat_completion_internal(unified_router, request, context).await
}

async fn handle_chat_completion_internal(
    unified_router: &crate::core::router::UnifiedRouter,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    // Keep provider selection for capability validation before execution.
    select_provider_for_model(
        unified_router,
        &request.model,
        ProviderCapability::ChatCompletion,
    )?;

    let requested_model = request.model.clone();
    let core_request = build_core_chat_request(request, requested_model, false)?;
    let requested_model = core_request.model.clone();
    let context_for_execution = context.clone();

    let execution = unified_router
        .execute_with_retry(&requested_model, move |deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            async move {
                let deployment =
                    unified_router
                        .get_deployment(&deployment_id)
                        .ok_or_else(|| {
                            ProviderError::other("router", "Selected deployment not found")
                        })?;

                let provider = deployment.provider.clone();
                let selected_model = deployment.model.clone();
                drop(deployment);

                let mut request_for_provider = core_request.clone();
                request_for_provider.model = selected_model;
                let response = provider
                    .chat_completion(request_for_provider, context)
                    .await?;
                let tokens = response
                    .usage
                    .as_ref()
                    .map(|usage| u64::from(usage.total_tokens))
                    .unwrap_or_default();
                Ok((response, tokens))
            }
        })
        .await
        .map_err(|(e, _)| GatewayError::Provider(e))?;

    Ok(convert_core_chat_response(execution.0))
}

pub(crate) fn build_core_chat_request(
    request: ChatCompletionRequest,
    model: String,
    stream: bool,
) -> Result<CoreChatRequest, GatewayError> {
    let tools = match request.tools {
        Some(tools) => {
            let mut converted = Vec::with_capacity(tools.len());
            for tool in tools {
                converted.push(convert_tool(tool)?);
            }
            Some(converted)
        }
        None => None,
    };

    let tool_choice = request.tool_choice.map(convert_tool_choice);

    let functions = match request.functions {
        Some(funcs) => {
            let mut values = Vec::with_capacity(funcs.len());
            for function in funcs {
                values.push(serde_json::to_value(function).map_err(|e| {
                    GatewayError::internal(format!("Failed to serialize function: {}", e))
                })?);
            }
            Some(values)
        }
        None => None,
    };

    let function_call = match request.function_call {
        Some(call) => Some(serde_json::to_value(call).map_err(|e| {
            GatewayError::internal(format!("Failed to serialize function call: {}", e))
        })?),
        None => None,
    };

    let response_format = request
        .response_format
        .map(|format| types::tools::ResponseFormat {
            format_type: format.format_type,
            json_schema: format.json_schema,
            response_type: None,
        });

    let mut extra_params = std::collections::HashMap::new();
    if let Some(modalities) = request.modalities {
        extra_params.insert("modalities".to_string(), json!(modalities));
    }
    if let Some(audio) = request.audio {
        extra_params.insert("audio".to_string(), json!(audio));
    }

    let stream_options = request
        .stream_options
        .map(|so| crate::core::types::chat::StreamOptions {
            include_usage: so.include_usage,
        });

    Ok(CoreChatRequest {
        model,
        messages: request.messages.into_iter().map(Into::into).collect(),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        max_completion_tokens: request.max_completion_tokens,
        top_p: request.top_p,
        frequency_penalty: request.frequency_penalty,
        presence_penalty: request.presence_penalty,
        stop: request.stop,
        stream,
        stream_options,
        tools,
        tool_choice,
        parallel_tool_calls: None,
        response_format,
        user: request.user,
        seed: request.seed.map(|s| s as i32),
        n: request.n,
        logit_bias: request.logit_bias,
        functions,
        function_call,
        logprobs: request.logprobs,
        top_logprobs: request.top_logprobs,
        thinking: None,
        reasoning_effort: request.reasoning_effort,
        store: request.store,
        metadata: request.metadata,
        service_tier: request.service_tier,
        extra_params,
    })
}

fn convert_core_chat_response(response: types::responses::ChatResponse) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: response.id,
        object: response.object,
        created: response.created as u64,
        model: response.model,
        system_fingerprint: response.system_fingerprint,
        choices: response
            .choices
            .into_iter()
            .map(|choice| ChatChoice {
                index: choice.index,
                message: choice.message.into(),
                logprobs: choice.logprobs.map(convert_logprobs),
                finish_reason: choice.finish_reason.map(convert_finish_reason),
            })
            .collect(),
        usage: response.usage.map(convert_usage),
    }
}

fn convert_tool(tool: Tool) -> Result<types::tools::Tool, GatewayError> {
    if tool.tool_type.to_lowercase() != "function" {
        return Err(GatewayError::validation("Unsupported tool type"));
    }

    Ok(types::tools::Tool {
        tool_type: types::tools::ToolType::Function,
        function: types::tools::FunctionDefinition {
            name: tool.function.name,
            description: tool.function.description,
            parameters: tool.function.parameters,
        },
    })
}

fn convert_tool_choice(choice: ToolChoice) -> types::tools::ToolChoice {
    match choice {
        ToolChoice::None(value) => types::tools::ToolChoice::String(value),
        ToolChoice::Auto(value) => types::tools::ToolChoice::String(value),
        ToolChoice::Required(value) => types::tools::ToolChoice::String(value),
        ToolChoice::Specific(spec) => types::tools::ToolChoice::Specific {
            choice_type: spec.tool_type,
            function: Some(types::tools::FunctionChoice {
                name: spec.function.name,
            }),
        },
    }
}

fn convert_logprobs(logprobs: types::responses::LogProbs) -> Logprobs {
    let content = if logprobs.content.is_empty() {
        None
    } else {
        Some(
            logprobs
                .content
                .into_iter()
                .map(|token| ContentLogprob {
                    token: token.token,
                    logprob: token.logprob,
                    bytes: token.bytes,
                    top_logprobs: token.top_logprobs.map(|tops| {
                        tops.into_iter()
                            .map(|top| TopLogprob {
                                token: top.token,
                                logprob: top.logprob,
                                bytes: top.bytes,
                            })
                            .collect()
                    }),
                })
                .collect(),
        )
    };

    Logprobs { content }
}

fn convert_finish_reason(reason: types::responses::FinishReason) -> String {
    match reason {
        types::responses::FinishReason::Stop => "stop",
        types::responses::FinishReason::Length => "length",
        types::responses::FinishReason::ToolCalls => "tool_calls",
        types::responses::FinishReason::ContentFilter => "content_filter",
        types::responses::FinishReason::FunctionCall => "function_call",
    }
    .to_string()
}

fn convert_usage(usage: types::responses::Usage) -> Usage {
    Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_tokens_details: usage.prompt_tokens_details.map(|details| {
            crate::core::models::openai::PromptTokensDetails {
                cached_tokens: details.cached_tokens,
                audio_tokens: details.audio_tokens,
            }
        }),
        completion_tokens_details: usage.completion_tokens_details.map(|details| {
            crate::core::models::openai::CompletionTokensDetails {
                reasoning_tokens: details.reasoning_tokens,
                audio_tokens: details.audio_tokens,
            }
        }),
    }
}

/// Format a provider error into SSE error type and code for OpenAI-compatible responses.
fn sse_error_classification(error: &ProviderError) -> (&'static str, &'static str) {
    match error {
        ProviderError::Authentication { .. } => ("invalid_request_error", "authentication_error"),
        ProviderError::RateLimit { .. } => ("rate_limit_error", "rate_limit_exceeded"),
        ProviderError::InvalidRequest { .. } => ("invalid_request_error", "invalid_request"),
        ProviderError::ModelNotFound { .. } => ("invalid_request_error", "model_not_found"),
        ProviderError::Timeout { .. } => ("server_error", "timeout"),
        ProviderError::ContentFiltered { .. } => ("invalid_request_error", "content_filter"),
        ProviderError::ContextLengthExceeded { .. } => {
            ("invalid_request_error", "context_length_exceeded")
        }
        ProviderError::TokenLimitExceeded { .. } => {
            ("invalid_request_error", "token_limit_exceeded")
        }
        _ => ("server_error", "internal_error"),
    }
}

/// Format an error as an SSE event matching OpenAI's streaming error format.
///
/// Produces:
/// ```text
/// data: {"error":{"message":"...","type":"server_error","code":"internal_error"}}
///
/// data: [DONE]
/// ```
fn format_sse_error(message: &str, error_type: &str, code: &str) -> Bytes {
    let error_json = json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": code,
        }
    });
    let error_event = Event::default().data(&error_json.to_string());
    let done_event = Event::default().data("[DONE]");
    let mut combined = error_event.to_bytes().to_vec();
    combined.extend_from_slice(&done_event.to_bytes());
    Bytes::from(combined)
}

fn convert_core_chunk_to_streaming(chunk: types::responses::ChatChunk) -> ChatCompletionChunk {
    ChatCompletionChunk {
        id: chunk.id,
        object: chunk.object,
        created: chunk.created as u64,
        model: chunk.model,
        system_fingerprint: chunk.system_fingerprint,
        choices: chunk
            .choices
            .into_iter()
            .map(|choice| ChatCompletionChunkChoice {
                index: choice.index,
                delta: ChatCompletionDelta {
                    role: choice.delta.role,
                    content: choice.delta.content,
                    tool_calls: choice
                        .delta
                        .tool_calls
                        .map(|calls| calls.into_iter().map(convert_tool_call_delta).collect()),
                },
                finish_reason: choice.finish_reason.map(convert_finish_reason),
                logprobs: choice
                    .logprobs
                    .and_then(|lp| serde_json::to_value(convert_logprobs(lp)).ok()),
            })
            .collect(),
        usage: chunk.usage.map(convert_usage),
    }
}

fn convert_tool_call_delta(
    delta: types::responses::ToolCallDelta,
) -> crate::core::streaming::types::ToolCallDelta {
    crate::core::streaming::types::ToolCallDelta {
        index: delta.index,
        id: delta.id,
        tool_type: delta.tool_type,
        function: delta.function.map(
            |function| crate::core::streaming::types::FunctionCallDelta {
                name: function.name,
                arguments: function.arguments,
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::{ChatMessage, MessageContent, MessageRole};

    #[test]
    fn test_convert_finish_reason() {
        assert_eq!(
            convert_finish_reason(types::responses::FinishReason::Stop),
            "stop"
        );
        assert_eq!(
            convert_finish_reason(types::responses::FinishReason::Length),
            "length"
        );
        assert_eq!(
            convert_finish_reason(types::responses::FinishReason::ToolCalls),
            "tool_calls"
        );
    }

    #[test]
    fn test_format_sse_error_produces_openai_format() {
        let bytes = format_sse_error("something went wrong", "server_error", "internal_error");
        let text = String::from_utf8(bytes.to_vec()).unwrap();

        // Should contain an error event followed by a DONE event
        assert!(text.contains("data: {"));
        assert!(text.contains("data: [DONE]"));

        // Extract the JSON from the first data line
        let first_data = text
            .lines()
            .find(|l| l.starts_with("data: {"))
            .unwrap()
            .strip_prefix("data: ")
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(first_data).unwrap();
        assert_eq!(parsed["error"]["message"], "something went wrong");
        assert_eq!(parsed["error"]["type"], "server_error");
        assert_eq!(parsed["error"]["code"], "internal_error");
    }

    #[test]
    fn test_sse_error_classification_auth() {
        let err = ProviderError::Authentication {
            provider: "openai",
            message: "bad key".to_string(),
        };
        let (t, c) = sse_error_classification(&err);
        assert_eq!(t, "invalid_request_error");
        assert_eq!(c, "authentication_error");
    }

    #[test]
    fn test_sse_error_classification_rate_limit() {
        let err = ProviderError::RateLimit {
            provider: "openai",
            message: "too many".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        };
        let (t, c) = sse_error_classification(&err);
        assert_eq!(t, "rate_limit_error");
        assert_eq!(c, "rate_limit_exceeded");
    }

    #[test]
    fn test_sse_error_classification_timeout() {
        let err = ProviderError::Timeout {
            provider: "openai",
            message: "timed out".to_string(),
        };
        let (t, c) = sse_error_classification(&err);
        assert_eq!(t, "server_error");
        assert_eq!(c, "timeout");
    }

    #[test]
    fn test_sse_error_classification_fallback() {
        let err = ProviderError::Network {
            provider: "openai",
            message: "dns failed".to_string(),
        };
        let (t, c) = sse_error_classification(&err);
        assert_eq!(t, "server_error");
        assert_eq!(c, "internal_error");
    }

    #[test]
    fn test_build_core_chat_request_minimal() {
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            ..Default::default()
        };

        let core_request = build_core_chat_request(request, "gpt-4".to_string(), false).unwrap();
        assert_eq!(core_request.model, "gpt-4");
        assert_eq!(core_request.messages.len(), 1);
    }
}
