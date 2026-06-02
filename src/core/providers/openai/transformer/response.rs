//! OpenAI Response Transformer
//!
//! Converts OpenAI-specific response formats into unified ChatResponse types.
//! Also handles streaming chunk transformation.

use crate::core::types::chat::ChatMessage;
use crate::core::types::message::MessageRole;
use crate::core::types::responses::{
    AudioDelta, ChatChoice, ChatChunk, ChatDelta, ChatResponse, ChatStreamChoice, FinishReason,
    FunctionCallDelta, LogProbs, TokenLogProb, ToolCallDelta, TopLogProb, Usage,
};
use crate::core::types::thinking::{ThinkingContent, ThinkingDelta};

use super::super::error::OpenAIError;
use super::super::models::*;

/// OpenAI Response Transformer
pub struct OpenAIResponseTransformer;

impl OpenAIResponseTransformer {
    /// Transform OpenAIChatResponse to ChatResponse
    pub fn transform(response: OpenAIChatResponse) -> Result<ChatResponse, OpenAIError> {
        let choices = response
            .choices
            .into_iter()
            .map(Self::transform_choice)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ChatResponse {
            id: response.id,
            object: response.object,
            created: response.created,
            model: response.model,
            choices,
            usage: response.usage.map(Self::transform_usage),
            system_fingerprint: response.system_fingerprint,
        })
    }

    /// Transform stream chunk
    pub fn transform_stream_chunk(chunk: OpenAIStreamChunk) -> Result<ChatChunk, OpenAIError> {
        let choices = chunk
            .choices
            .into_iter()
            .map(Self::transform_stream_choice)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ChatChunk {
            id: chunk.id,
            object: chunk.object,
            created: chunk.created,
            model: chunk.model,
            choices,
            usage: chunk.usage.map(Self::transform_usage),
            system_fingerprint: chunk.system_fingerprint,
        })
    }

    /// Transform stream choice
    fn transform_stream_choice(
        choice: OpenAIStreamChoice,
    ) -> Result<ChatStreamChoice, OpenAIError> {
        Ok(ChatStreamChoice {
            index: choice.index,
            delta: Self::transform_delta(choice.delta)?,
            logprobs: choice.logprobs.and_then(|lp| {
                serde_json::from_value::<OpenAILogprobs>(lp)
                    .ok()
                    .map(Self::transform_logprobs)
            }),
            finish_reason: choice.finish_reason.map(Self::transform_finish_reason),
        })
    }

    /// Transform delta
    fn transform_delta(delta: OpenAIDelta) -> Result<ChatDelta, OpenAIError> {
        let tool_calls = delta.tool_calls.map(|calls| {
            calls
                .into_iter()
                .map(|tc| ToolCallDelta {
                    index: tc.index,
                    id: tc.id,
                    tool_type: tc.tool_type,
                    function: tc.function.map(|f| FunctionCallDelta {
                        name: f.name,
                        arguments: f.arguments,
                    }),
                })
                .collect()
        });
        let function_call = delta.function_call.map(|f| FunctionCallDelta {
            name: f.name,
            arguments: f.arguments,
        });
        let thinking = delta
            .reasoning_content
            .filter(|reasoning| !reasoning.is_empty())
            .or(delta.reasoning)
            .filter(|reasoning| !reasoning.is_empty())
            .map(ThinkingDelta::new);
        let audio = delta.audio.map(|a| AudioDelta {
            id: a.id,
            expires_at: a.expires_at,
            data: a.data,
            transcript: a.transcript,
            format: a.format,
        });

        Ok(ChatDelta {
            role: delta.role.map(|r| match r.as_str() {
                "system" => MessageRole::System,
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                "tool" => MessageRole::Tool,
                "function" => MessageRole::Function,
                _ => MessageRole::Assistant,
            }),
            content: delta.content,
            thinking,
            tool_calls,
            function_call,
            audio,
            ..Default::default()
        })
    }

    /// Transform choice
    fn transform_choice(choice: OpenAIChoice) -> Result<ChatChoice, OpenAIError> {
        Ok(ChatChoice {
            index: choice.index,
            message: Self::transform_message_response(choice.message)?,
            logprobs: choice.logprobs.and_then(|lp| {
                serde_json::from_value::<OpenAILogprobs>(lp)
                    .ok()
                    .map(Self::transform_logprobs)
            }),
            finish_reason: choice.finish_reason.map(Self::transform_finish_reason),
        })
    }

    /// Transform message response
    fn transform_message_response(message: OpenAIMessage) -> Result<ChatMessage, OpenAIError> {
        let audio = message
            .audio
            .clone()
            .and_then(OpenAIMessageAudio::into_core_audio);
        // Extract thinking content from reasoning fields
        // Priority: reasoning_content (DeepSeek) > reasoning (OpenAI)
        let thinking = message
            .reasoning_content
            .as_ref()
            .filter(|s| !s.is_empty())
            .or(message.reasoning.as_ref().filter(|s| !s.is_empty()))
            .map(|text| ThinkingContent::Text {
                text: text.clone(),
                signature: None,
            });
        let compatible_message =
            message
                .into_compatible_message()
                .map_err(|message| OpenAIError::ResponseParsing {
                    provider: "openai",
                    message,
                })?;
        let mut core_message: ChatMessage = compatible_message.into();
        core_message.thinking = thinking;
        core_message.audio = audio;
        Ok(core_message)
    }

    /// Transform usage
    pub(super) fn transform_usage(usage: OpenAIUsage) -> Usage {
        Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            thinking_usage: None,
            prompt_tokens_details: usage.prompt_tokens_details.map(|details| {
                crate::core::types::responses::PromptTokensDetails {
                    cached_tokens: details.cached_tokens,
                    cache_creation_tokens: None,
                    cache_read_tokens: details.cached_tokens,
                    audio_tokens: details.audio_tokens,
                }
            }),
            completion_tokens_details: usage.completion_tokens_details.map(|details| {
                crate::core::types::responses::CompletionTokensDetails {
                    reasoning_tokens: details.reasoning_tokens,
                    audio_tokens: details.audio_tokens,
                }
            }),
        }
    }

    /// Transform logprobs
    pub(super) fn transform_logprobs(logprobs: OpenAILogprobs) -> LogProbs {
        LogProbs {
            content: logprobs
                .content
                .map(|content| {
                    content
                        .into_iter()
                        .map(|token| TokenLogProb {
                            token: token.token,
                            logprob: token.logprob,
                            bytes: token.bytes,
                            top_logprobs: Some(
                                token
                                    .top_logprobs
                                    .into_iter()
                                    .map(|top| TopLogProb {
                                        token: top.token,
                                        logprob: top.logprob,
                                        bytes: top.bytes,
                                    })
                                    .collect(),
                            ),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            refusal: logprobs.refusal.map(|_| "filtered".to_string()),
        }
    }

    /// Transform finish reason
    pub(super) fn transform_finish_reason(reason: String) -> FinishReason {
        match reason.as_str() {
            "stop" => FinishReason::Stop,
            "length" => FinishReason::Length,
            "function_call" => FinishReason::FunctionCall,
            "tool_calls" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Stop,
        }
    }
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod response_tests;
