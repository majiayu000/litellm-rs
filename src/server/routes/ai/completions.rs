//! Legacy OpenAI text completions compatibility route.

use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, warn};

use crate::core::models::openai::{
    ChatCompletionRequest, ChatMessage, CompletionChoice, CompletionResponse, MessageContent,
    MessageRole, StreamOptions, Usage,
};
use crate::core::types;
use crate::server::state::AppState;
use crate::utils::data::validation::RequestValidator;
use crate::utils::error::gateway_error::GatewayError;

use super::openai_errors;
#[path = "completions_sse.rs"]
mod completions_sse;
#[path = "completions_streaming.rs"]
mod completions_streaming;

#[derive(Debug, Clone)]
struct CompletionAdapterRequest {
    chat_request: ChatCompletionRequest,
    prompt: String,
    echo: bool,
    stream: bool,
    include_usage: bool,
}

#[derive(Debug, Serialize)]
struct CompletionSseChunk {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<CompletionChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<Usage>,
}

pub async fn completions(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<Value>,
) -> ActixResult<HttpResponse> {
    completions_inner(state, req, None, request.into_inner()).await
}

pub async fn engine_completions(
    state: web::Data<AppState>,
    req: HttpRequest,
    model: web::Path<String>,
    request: web::Json<Value>,
) -> ActixResult<HttpResponse> {
    completions_inner(state, req, Some(model.into_inner()), request.into_inner()).await
}

async fn completions_inner(
    state: web::Data<AppState>,
    req: HttpRequest,
    path_model: Option<String>,
    request: Value,
) -> ActixResult<HttpResponse> {
    let context = match super::token_policy::shared_request_context_with_api_key_token_limit(&req) {
        Ok(context) => context,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };
    let adapter_request = match completion_request_from_value(request, path_model) {
        Ok(request) => request,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    if let Err(error) = RequestValidator::validate_chat_completion_request(
        &adapter_request.chat_request.model,
        &adapter_request.chat_request.messages,
        adapter_request.chat_request.max_tokens,
        adapter_request.chat_request.temperature,
    ) {
        warn!("Invalid completions request: {}", error);
        return Ok(openai_errors::validation_error(error.to_string()));
    }
    if let Err(error) = super::context::enforce_api_key_model_and_token_limits(
        &req,
        &adapter_request.chat_request.model,
        super::token_policy::requested_chat_output_token_limit(&adapter_request.chat_request),
    ) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    if adapter_request.stream {
        return completions_streaming::handle_streaming_completion(
            state.get_ref(),
            adapter_request,
            context,
        )
        .await;
    }

    let chat_request = match crate::server::guardrails::apply_chat_input(
        state.get_ref(),
        &adapter_request.chat_request,
    )
    .await
    {
        Ok(request) => request,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };
    let masked_prompt = match guarded_completion_prompt(&chat_request) {
        Ok(prompt) => prompt,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    match super::chat::handle_chat_completion_after_input_guardrail_deferred(
        state.get_ref(),
        Arc::new(chat_request),
        context,
    )
    .await
    {
        Ok((response, callback)) => {
            let response = if adapter_request.echo {
                let echoed = chat_response_with_completion_echo(response, &masked_prompt);
                match crate::server::guardrails::apply_output_with_engine(
                    state.guardrails().as_ref(),
                    &echoed,
                )
                .await
                {
                    Ok(response) => response,
                    Err(error) => {
                        callback.fail(error.to_string(), "guardrail_output");
                        return Ok(openai_errors::gateway_error_response(&error));
                    }
                }
            } else {
                response
            };
            callback.complete_usage(response.usage.as_ref(), "success");
            Ok(HttpResponse::Ok().json(completion_response_from_chat(response, "", false)))
        }
        Err(error) => {
            error!("Completion error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

fn completion_request_from_value(
    body: Value,
    path_model: Option<String>,
) -> Result<CompletionAdapterRequest, GatewayError> {
    let object = body
        .as_object()
        .ok_or_else(|| GatewayError::validation("request body must be a JSON object"))?;
    reject_unsupported_fields(object)?;

    let model = match path_model {
        Some(model) => model,
        None => required_string_field(object, "model", "Model is required")?,
    };
    let prompt = prompt_field(object.get("prompt"))?;
    let stream = bool_field(object, "stream")?.unwrap_or(false);
    let stream_options = object.get("stream_options");
    let include_usage = include_usage_field(stream_options)?.unwrap_or(false);
    if stream_options.is_some() && !stream {
        return Err(GatewayError::validation(
            "stream_options is only supported when stream is true",
        ));
    }
    let echo = bool_field(object, "echo")?.unwrap_or(false);
    let logprobs = u32_field(object, "logprobs")?;
    if logprobs.is_some() {
        return Err(GatewayError::validation(
            "logprobs is not supported for /v1/completions compatibility",
        ));
    }

    Ok(CompletionAdapterRequest {
        prompt: prompt.clone(),
        echo,
        stream,
        include_usage,
        chat_request: ChatCompletionRequest {
            model,
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text(prompt)),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            max_tokens: u32_field(object, "max_tokens")?,
            temperature: f32_field(object, "temperature")?,
            top_p: f32_field(object, "top_p")?,
            n: u32_field(object, "n")?,
            stream: Some(stream),
            stream_options: stream.then_some(StreamOptions {
                include_usage: Some(include_usage),
            }),
            stop: stop_field(object.get("stop"))?,
            presence_penalty: f32_field(object, "presence_penalty")?,
            frequency_penalty: f32_field(object, "frequency_penalty")?,
            logit_bias: logit_bias_field(object.get("logit_bias"))?,
            user: optional_string_field(object, "user")?,
            ..ChatCompletionRequest::default()
        },
    })
}

fn reject_unsupported_fields(object: &serde_json::Map<String, Value>) -> Result<(), GatewayError> {
    const ALLOWED: &[&str] = &[
        "model",
        "prompt",
        "max_tokens",
        "temperature",
        "top_p",
        "n",
        "stream",
        "stream_options",
        "stop",
        "presence_penalty",
        "frequency_penalty",
        "logit_bias",
        "user",
        "logprobs",
        "echo",
    ];
    const UNSUPPORTED: &[&str] = &["best_of", "suffix"];
    for key in object.keys() {
        let key_str = key.as_str();
        if UNSUPPORTED.contains(&key_str) {
            return Err(GatewayError::validation(format!(
                "Unsupported /v1/completions field: {key}"
            )));
        }
        if !ALLOWED.contains(&key_str) {
            return Err(GatewayError::validation(format!(
                "Unknown /v1/completions field: {key}"
            )));
        }
    }
    Ok(())
}

fn prompt_field(value: Option<&Value>) -> Result<String, GatewayError> {
    match value {
        Some(Value::String(prompt)) => Ok(prompt.clone()),
        Some(Value::Array(items)) if items.len() == 1 => items[0]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| GatewayError::validation("prompt array must contain strings")),
        Some(Value::Array(_)) => Err(GatewayError::validation(
            "prompt arrays with multiple entries are not supported",
        )),
        Some(_) => Err(GatewayError::validation("prompt must be a string")),
        None => Err(GatewayError::validation("prompt is required")),
    }
}

fn stop_field(value: Option<&Value>) -> Result<Option<Vec<String>>, GatewayError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(stop)) => Ok(Some(vec![stop.clone()])),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| GatewayError::validation("stop entries must be strings"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(GatewayError::validation(
            "stop must be a string or string array",
        )),
    }
}

fn logit_bias_field(value: Option<&Value>) -> Result<Option<HashMap<String, f32>>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let map = value
        .as_object()
        .ok_or_else(|| GatewayError::validation("logit_bias must be an object"))?;
    let mut result = HashMap::with_capacity(map.len());
    for (key, value) in map {
        let Some(value) = value.as_f64() else {
            return Err(GatewayError::validation(
                "logit_bias values must be numbers",
            ));
        };
        if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
            return Err(GatewayError::validation("logit_bias value is out of range"));
        }
        result.insert(key.clone(), value as f32);
    }
    Ok(Some(result))
}

fn required_string_field(
    object: &serde_json::Map<String, Value>,
    key: &str,
    missing_message: &'static str,
) -> Result<String, GatewayError> {
    match object.get(key) {
        Some(Value::String(value)) => Ok(value.trim().to_string()),
        Some(_) => Err(GatewayError::validation(format!("{key} must be a string"))),
        None => Err(GatewayError::validation(missing_message)),
    }
}

fn optional_string_field(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, GatewayError> {
    match object.get(key) {
        Some(Value::String(value)) => Ok(Some(value.trim().to_string())),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(GatewayError::validation(format!("{key} must be a string"))),
    }
}

fn bool_field(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, GatewayError> {
    match object.get(key) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(GatewayError::validation(format!("{key} must be a boolean"))),
    }
}

fn u32_field(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<u32>, GatewayError> {
    match object.get(key) {
        Some(Value::Number(value)) => {
            let Some(value) = value.as_u64() else {
                return Err(GatewayError::validation(format!(
                    "{key} must be a non-negative integer"
                )));
            };
            u32::try_from(value).map(Some).map_err(|_| {
                GatewayError::validation(format!("{key} must fit in an unsigned 32-bit integer"))
            })
        }
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(GatewayError::validation(format!(
            "{key} must be an integer"
        ))),
    }
}

fn include_usage_field(value: Option<&Value>) -> Result<Option<bool>, GatewayError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(map)) => match map.get("include_usage") {
            Some(Value::Bool(value)) => Ok(Some(*value)),
            Some(Value::Null) | None => Ok(None),
            Some(_) => Err(GatewayError::validation(
                "stream_options.include_usage must be a boolean",
            )),
        },
        Some(_) => Err(GatewayError::validation("stream_options must be an object")),
    }
}

fn f32_field(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<f32>, GatewayError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_f64() else {
        return Err(GatewayError::validation(format!("{key} must be a number")));
    };
    if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
        return Err(GatewayError::validation(format!("{key} is out of range")));
    }
    Ok(Some(value as f32))
}

fn completion_response_from_chat(
    response: crate::core::models::openai::ChatCompletionResponse,
    prompt: &str,
    echo: bool,
) -> CompletionResponse {
    CompletionResponse {
        id: response.id,
        object: "text_completion".to_string(),
        created: response.created,
        model: response.model,
        choices: response
            .choices
            .into_iter()
            .map(|choice| CompletionChoice {
                text: completion_text_from_message(choice.message.content.as_ref(), prompt, echo),
                index: choice.index,
                logprobs: choice
                    .logprobs
                    .and_then(|logprobs| serde_json::to_value(logprobs).ok()),
                finish_reason: choice.finish_reason,
            })
            .collect(),
        usage: response.usage,
    }
}

fn guarded_completion_prompt(request: &ChatCompletionRequest) -> Result<String, GatewayError> {
    let content = request
        .messages
        .first()
        .and_then(|message| message.content.as_ref())
        .ok_or_else(|| {
            GatewayError::internal(
                "completion input guardrail projection produced no prompt content",
            )
        })?;
    Ok(match content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                crate::core::models::openai::ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

fn chat_response_with_completion_echo(
    mut response: crate::core::models::openai::ChatCompletionResponse,
    prompt: &str,
) -> crate::core::models::openai::ChatCompletionResponse {
    for choice in &mut response.choices {
        let text = completion_text_from_message(choice.message.content.as_ref(), prompt, true);
        choice.message.content = Some(MessageContent::Text(text));
    }
    response
}

fn completion_chunk_from_core(
    chunk: types::responses::ChatChunk,
    echo_prefix: Option<&str>,
    include_usage: bool,
) -> CompletionSseChunk {
    CompletionSseChunk {
        id: chunk.id,
        object: "text_completion",
        created: chunk.created as u64,
        model: chunk.model,
        choices: chunk
            .choices
            .into_iter()
            .map(|choice| CompletionChoice {
                text: completion_text_from_delta(choice.delta.content, echo_prefix),
                index: choice.index,
                logprobs: choice
                    .logprobs
                    .and_then(|logprobs| serde_json::to_value(logprobs).ok()),
                finish_reason: choice.finish_reason.map(finish_reason_to_string),
            })
            .collect(),
        usage: include_usage.then_some(chunk.usage).flatten(),
    }
}

fn chunk_has_text_delta(chunk: &types::responses::ChatChunk) -> bool {
    chunk.choices.iter().any(|choice| {
        choice
            .delta
            .content
            .as_deref()
            .is_some_and(|text| !text.is_empty())
    })
}

fn completion_text_from_message(
    content: Option<&MessageContent>,
    prompt: &str,
    echo: bool,
) -> String {
    let completion = match content {
        Some(MessageContent::Text(text)) => text.clone(),
        Some(MessageContent::Parts(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                crate::core::models::openai::ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        None => String::new(),
    };
    if echo {
        format!("{prompt}{completion}")
    } else {
        completion
    }
}

fn completion_text_from_delta(content: Option<String>, echo_prefix: Option<&str>) -> String {
    let content = content.unwrap_or_default();
    match echo_prefix {
        Some(prefix) => format!("{prefix}{content}"),
        None => content,
    }
}

fn finish_reason_to_string(reason: types::responses::FinishReason) -> String {
    match reason {
        types::responses::FinishReason::Stop => "stop",
        types::responses::FinishReason::Length => "length",
        types::responses::FinishReason::ToolCalls => "tool_calls",
        types::responses::FinishReason::ContentFilter => "content_filter",
        types::responses::FinishReason::FunctionCall => "function_call",
        types::responses::FinishReason::StopSequence => "stop_sequence",
        types::responses::FinishReason::Refusal => "refusal",
        types::responses::FinishReason::PauseTurn => "pause_turn",
    }
    .to_string()
}

#[cfg(test)]
#[path = "completions_tests.rs"]
mod tests;
