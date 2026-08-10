//! Canonical runtime adapter for [`super::LLMClient`].

use super::LLMClient;
use crate::core::router::RuntimeHandle;
use crate::core::types::chat::{ChatMessage as CoreMessage, ChatRequest as CoreChatRequest};
use crate::core::types::context::RequestContext;
use crate::core::types::responses::{
    ChatChunk as CoreChatChunk, ChatResponse as CoreChatResponse, FinishReason,
};
use crate::core::types::tools::{
    FunctionCall as CoreFunctionCall, FunctionDefinition, Tool as CoreTool,
    ToolCall as CoreToolCall, ToolChoice as CoreToolChoice, ToolType,
};
use crate::sdk::errors::{Result, SDKError};
use crate::sdk::types::{
    ChatChoice, ChatChunk, ChatResponse, ChunkChoice, Function as SdkFunction, Message,
    MessageDelta, SdkChatRequest, ToolCall as SdkToolCall, ToolChoice as SdkToolChoice, Usage,
};
use futures::StreamExt;
use std::pin::Pin;

impl LLMClient {
    fn runtime_handle(&self) -> Result<RuntimeHandle> {
        self.runtime_binding
            .as_ref()
            .map(|binding| binding.bind())
            .ok_or_else(|| SDKError::ConfigError("canonical runtime is not configured".to_string()))
    }

    fn runtime_model<'a>(&'a self, requested: &'a str) -> Result<&'a str> {
        if requested.is_empty() {
            self.runtime_default_model.as_deref().ok_or_else(|| {
                SDKError::ConfigError(
                    "canonical runtime default model is not configured".to_string(),
                )
            })
        } else {
            Ok(requested)
        }
    }

    pub(super) async fn chat_with_runtime(&self, request: SdkChatRequest) -> Result<ChatResponse> {
        let model = self.runtime_model(&request.model)?.to_string();
        let core_request = sdk_request_to_core(&model, request)?;
        let context = RequestContext::new();
        let execution = self
            .runtime_handle()?
            .execute_with_selected_deployment_typed(&model, move |deployment| {
                let mut request = core_request.clone();
                let context = context.clone();
                async move {
                    request.model = deployment.model.clone();
                    let response = deployment
                        .provider
                        .chat_completion(request, context)
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
            .map_err(SDKError::from)?;

        core_response_to_sdk(execution.result)
    }

    pub(super) async fn chat_stream_with_runtime(
        &self,
        messages: Vec<Message>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<ChatChunk>> + Send>>> {
        let model = self.runtime_model("")?.to_string();
        let request = SdkChatRequest {
            model: model.clone(),
            messages,
            options: crate::sdk::types::ChatOptions {
                stream: true,
                ..Default::default()
            },
        };
        let core_request = sdk_request_to_core(&model, request)?;
        let context = RequestContext::new();
        let handle = self.runtime_handle()?;
        let lease = handle
            .select_deployment_lease_typed(&model)
            .map_err(SDKError::from)?;
        let deployment = lease.clone_deployment();
        let mut core_request = core_request;
        core_request.model = deployment.model.clone();
        let stream = deployment
            .provider
            .chat_completion_stream(core_request, context)
            .await
            .map_err(SDKError::from)?;

        Ok(Box::pin(stream.map(move |chunk| {
            let _lease = &lease;
            chunk.map_err(SDKError::from).and_then(core_chunk_to_sdk)
        })))
    }
}

fn sdk_request_to_core(model: &str, request: SdkChatRequest) -> Result<CoreChatRequest> {
    let messages = request
        .messages
        .into_iter()
        .map(sdk_message_to_core)
        .collect::<Result<Vec<CoreMessage>>>()?;
    let tools = request
        .options
        .tools
        .map(|tools| {
            tools
                .into_iter()
                .map(|tool| {
                    if !tool.tool_type.eq_ignore_ascii_case("function") {
                        return Err(SDKError::NotSupported(format!(
                            "canonical runtime does not support SDK tool type '{}'",
                            tool.tool_type
                        )));
                    }
                    Ok(CoreTool {
                        tool_type: ToolType::Function,
                        function: FunctionDefinition {
                            name: tool.function.name,
                            description: tool.function.description,
                            parameters: Some(tool.function.parameters),
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let tool_choice = request
        .options
        .tool_choice
        .map(sdk_tool_choice_to_core)
        .transpose()?;

    Ok(CoreChatRequest {
        model: model.to_string(),
        messages,
        temperature: request.options.temperature,
        max_tokens: request.options.max_tokens,
        top_p: request.options.top_p,
        frequency_penalty: request.options.frequency_penalty,
        presence_penalty: request.options.presence_penalty,
        stop: request.options.stop,
        stream: request.options.stream,
        tools,
        tool_choice,
        ..Default::default()
    })
}

fn core_response_to_sdk(response: CoreChatResponse) -> Result<ChatResponse> {
    let created = u64::try_from(response.created)
        .map_err(|_| SDKError::ParseError("negative response timestamp".to_string()))?;
    let choices = response
        .choices
        .into_iter()
        .map(|choice| {
            Ok(ChatChoice {
                index: choice.index,
                message: core_message_to_sdk(choice.message)?,
                finish_reason: choice.finish_reason.map(finish_reason_name),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let usage = response.usage.unwrap_or_default();

    Ok(ChatResponse {
        id: response.id,
        model: response.model,
        choices,
        usage: Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        },
        created,
    })
}

fn core_chunk_to_sdk(chunk: CoreChatChunk) -> Result<ChatChunk> {
    let choices = chunk
        .choices
        .into_iter()
        .map(|choice| {
            if choice.delta.thinking.is_some()
                || choice.delta.tool_calls.is_some()
                || choice.delta.function_call.is_some()
                || choice.delta.audio.is_some()
            {
                return Err(SDKError::NotSupported(
                    "SDK ChatChunk cannot represent canonical thinking, tool, function, or audio deltas"
                        .to_string(),
                ));
            }
            Ok(ChunkChoice {
                index: choice.index,
                delta: MessageDelta {
                    role: choice.delta.role.map(transcode).transpose()?,
                    content: choice.delta.content,
                    tool_calls: None,
                },
                finish_reason: choice.finish_reason.map(finish_reason_name),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ChatChunk {
        id: chunk.id,
        model: chunk.model,
        choices,
    })
}

fn sdk_message_to_core(message: Message) -> Result<CoreMessage> {
    let tool_calls = message.tool_calls.map(|calls| {
        calls
            .into_iter()
            .map(|call| CoreToolCall {
                id: call.id,
                tool_type: call.tool_type,
                function: CoreFunctionCall {
                    name: call.function.name,
                    arguments: call.function.arguments.unwrap_or_default(),
                },
            })
            .collect()
    });

    Ok(CoreMessage {
        role: transcode(message.role)?,
        content: message.content.map(transcode).transpose()?,
        name: message.name,
        tool_calls,
        ..Default::default()
    })
}

fn core_message_to_sdk(message: CoreMessage) -> Result<Message> {
    if message.thinking.is_some()
        || message.audio.is_some()
        || message.tool_call_id.is_some()
        || message.function_call.is_some()
    {
        return Err(SDKError::NotSupported(
            "SDK Message cannot represent canonical thinking, audio, tool-result, or legacy function-call fields"
                .to_string(),
        ));
    }
    let tool_calls = message.tool_calls.map(|calls| {
        calls
            .into_iter()
            .map(|call| SdkToolCall {
                id: call.id,
                tool_type: call.tool_type,
                function: SdkFunction {
                    name: call.function.name,
                    description: None,
                    parameters: serde_json::Value::Null,
                    arguments: Some(call.function.arguments),
                },
            })
            .collect()
    });

    Ok(Message {
        role: transcode(message.role)?,
        content: message.content.map(transcode).transpose()?,
        name: message.name,
        tool_calls,
    })
}

fn sdk_tool_choice_to_core(choice: SdkToolChoice) -> Result<CoreToolChoice> {
    match choice {
        SdkToolChoice::None => Ok(CoreToolChoice::String("none".to_string())),
        SdkToolChoice::Auto => Ok(CoreToolChoice::String("auto".to_string())),
        SdkToolChoice::Required => Ok(CoreToolChoice::String("required".to_string())),
        SdkToolChoice::Function { name } => transcode(serde_json::json!({
            "type": "function",
            "function": { "name": name }
        })),
    }
}

fn finish_reason_name(reason: FinishReason) -> String {
    match reason {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::ContentFilter => "content_filter",
        FinishReason::FunctionCall => "function_call",
        FinishReason::StopSequence => "stop_sequence",
        FinishReason::Refusal => "refusal",
        FinishReason::PauseTurn => "pause_turn",
    }
    .to_string()
}

fn transcode<T, U>(value: T) -> Result<U>
where
    T: serde::Serialize,
    U: serde::de::DeserializeOwned,
{
    serde_json::from_value(serde_json::to_value(value)?).map_err(SDKError::from)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
