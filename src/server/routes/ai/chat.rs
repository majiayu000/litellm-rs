//! Chat completions endpoint

use crate::core::models::openai::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ContentLogprob, Logprobs, Tool,
    ToolChoice, TopLogprob, Usage,
};
use crate::core::providers::ProviderRegistry;
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
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use futures::StreamExt;
use serde_json::json;
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
        match handle_chat_completion_via_pool(&state.router, request.into_inner(), context).await {
            Ok(response) => Ok(HttpResponse::Ok().json(response)),
            Err(e) => {
                error!("Chat completion error: {}", e);
                Ok(errors::gateway_error_to_response(e))
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

    let selection = match select_provider_for_model(
        &state.router,
        &request.model,
        ProviderCapability::ChatCompletionStream,
    ) {
        Ok(selection) => selection,
        Err(e) => return Ok(errors::gateway_error_to_response(e)),
    };

    let core_request = match build_core_chat_request(request, selection.model.clone(), true) {
        Ok(req) => req,
        Err(e) => return Ok(errors::gateway_error_to_response(e)),
    };

    match selection
        .provider
        .chat_completion_stream(core_request, context)
        .await
    {
        Ok(mut stream) => {
            let sse_stream = async_stream::stream! {
                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            let chat_chunk = convert_core_chunk_to_streaming(chunk);
                            match serde_json::to_string(&chat_chunk) {
                                Ok(json) => {
                                    let event = Event::default().data(&json);
                                    yield Ok::<_, GatewayError>(event.to_bytes());
                                }
                                Err(e) => {
                                    yield Err(GatewayError::internal(format!("Serialization error: {}", e)));
                                }
                            }
                        }
                        Err(e) => {
                            error!("Stream chunk error: {}", e);
                            yield Err(GatewayError::internal(format!("Stream chunk error: {}", e)));
                        }
                    }
                }

                let done_event = Event::default().data("[DONE]");
                yield Ok::<_, GatewayError>(done_event.to_bytes());
            };

            Ok(HttpResponse::Ok()
                .insert_header((CONTENT_TYPE, "text/event-stream"))
                .insert_header((CACHE_CONTROL, "no-cache"))
                .insert_header(("Connection", "keep-alive"))
                .streaming(sse_stream))
        }
        Err(e) => {
            error!("Failed to create streaming response: {}", e);
            Ok(errors::gateway_error_to_response(GatewayError::internal(
                format!("Streaming error: {}", e),
            )))
        }
    }
}

/// Handle chat completion via provider pool
pub async fn handle_chat_completion_via_pool(
    pool: &ProviderRegistry,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    let selection =
        select_provider_for_model(pool, &request.model, ProviderCapability::ChatCompletion)?;

    let core_request = build_core_chat_request(request, selection.model.clone(), false)?;
    let core_response = selection
        .provider
        .chat_completion(core_request, context)
        .await
        .map_err(|e| GatewayError::internal(format!("Chat completion error: {}", e)))?;

    Ok(convert_core_chat_response(core_response))
}

fn build_core_chat_request(
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
