//! Legacy OpenAI text completions compatibility route.

use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use bytes::Bytes;
use futures::StreamExt;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::core::models::openai::{
    ChatCompletionRequest, ChatMessage, CompletionChoice, CompletionResponse, MessageContent,
    MessageRole, StreamOptions, Usage,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::{self, context::RequestContext, model::ProviderCapability};
use crate::server::state::AppState;
use crate::utils::data::validation::RequestValidator;
use crate::utils::error::gateway_error::GatewayError;

use super::context::get_request_context;
use super::execution::execute_stream_with_selected_deployment;
use super::openai_errors;

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
    let context = get_request_context(&req)?;
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

    if adapter_request.stream {
        return handle_streaming_completion(state.get_ref(), adapter_request, context).await;
    }

    match super::chat::handle_chat_completion_with_state(
        state.get_ref(),
        adapter_request.chat_request,
        context,
    )
    .await
    {
        Ok(response) => Ok(HttpResponse::Ok().json(completion_response_from_chat(
            response,
            &adapter_request.prompt,
            adapter_request.echo,
        ))),
        Err(error) => {
            error!("Completion error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

async fn handle_streaming_completion(
    state: &AppState,
    adapter_request: CompletionAdapterRequest,
    context: RequestContext,
) -> ActixResult<HttpResponse> {
    info!(
        "Handling streaming text completion for model: {}",
        adapter_request.chat_request.model
    );

    let mut request = adapter_request.chat_request;
    let requested_model = request.model.clone();
    let request_for_budget = request.clone();
    request
        .stream_options
        .get_or_insert(StreamOptions {
            include_usage: None,
        })
        .include_usage = Some(true);

    let core_request =
        match super::chat::build_core_chat_request(request, requested_model.clone(), true) {
            Ok(request) => request,
            Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
        };

    let context_for_execution = context.clone();
    let (budget_limits, pricing_service) = (state.budget_limits.clone(), state.pricing.clone());
    let api_key_id = context.api_key_id();

    match execute_stream_with_selected_deployment(
        state.unified_router.clone(),
        &requested_model,
        ProviderCapability::ChatCompletionStream,
        move |provider, selected_model, _selected_deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            let (budget_limits, pricing_service) = (budget_limits.clone(), pricing_service.clone());
            let request_for_budget = request_for_budget.clone();
            async move {
                super::spend::ensure_budget_available(
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                )?;
                let budget_reservation = super::spend::reserve_chat_completion_budget_with_pricing(
                    pricing_service.as_ref(),
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                    &request_for_budget,
                )?;
                let provider_name = provider.name().to_string();
                let mut request_for_provider = core_request.clone();
                request_for_provider.model = selected_model.clone();
                let stream = provider
                    .chat_completion_stream(request_for_provider, context)
                    .await?;
                Ok((stream, provider_name, selected_model, budget_reservation))
            }
        },
    )
    .await
    {
        Ok(((mut stream, served_provider, served_model, mut budget_reservation), lease)) => {
            let (tx, rx) = mpsc::channel::<Bytes>(8);
            let idle_timeout_secs = state.config.load().gateway.server.stream_idle_timeout;
            let budget_limits = state.budget_limits.clone();
            let key_manager = state.key_manager.clone();
            let pricing_service = state.pricing.clone();
            let include_usage = adapter_request.include_usage;
            let mut echo_prefix = adapter_request.echo.then_some(adapter_request.prompt);

            tokio::spawn(async move {
                let mut lease = Some(lease);
                let mut tokens_used = 0_u64;
                let mut final_usage = None;
                let mut saw_upstream_output = false;

                loop {
                    let chunk_result = if idle_timeout_secs == 0 {
                        stream.next().await
                    } else {
                        match tokio::time::timeout(
                            Duration::from_secs(idle_timeout_secs),
                            stream.next(),
                        )
                        .await
                        {
                            Ok(result) => result,
                            Err(_) => {
                                warn!(
                                    "Completion SSE stream idle timeout after {}s",
                                    idle_timeout_secs
                                );
                                send_stream_error(
                                    &tx,
                                    "Stream idle timeout",
                                    "server_error",
                                    "timeout",
                                )
                                .await;
                                if let Some(lease) = lease.take() {
                                    let error = ProviderError::timeout(
                                        "router",
                                        format!("stream idle timeout after {}s", idle_timeout_secs),
                                    );
                                    lease.finish_failure(&error);
                                }
                                settle_stream_spend_if_chargeable(
                                    pricing_service.as_ref(),
                                    &budget_limits,
                                    &key_manager,
                                    api_key_id,
                                    &served_provider,
                                    &served_model,
                                    final_usage.as_ref(),
                                    budget_reservation.take(),
                                    saw_upstream_output,
                                )
                                .await;
                                return;
                            }
                        }
                    };

                    let Some(chunk_result) = chunk_result else {
                        break;
                    };

                    let bytes = match chunk_result {
                        Ok(chunk) => {
                            saw_upstream_output = true;
                            if let Some(usage) = &chunk.usage {
                                tokens_used = u64::from(usage.total_tokens);
                                final_usage = Some(usage.clone());
                            }
                            let prefix_for_chunk = if chunk_has_text_delta(&chunk) {
                                echo_prefix.take()
                            } else {
                                None
                            };
                            let completion_chunk = completion_chunk_from_core(
                                chunk,
                                prefix_for_chunk.as_deref(),
                                include_usage,
                            );
                            if !include_usage && completion_chunk.choices.is_empty() {
                                continue;
                            }
                            match serde_json::to_string(&completion_chunk) {
                                Ok(json) => Event::default().data(&json).to_bytes(),
                                Err(error) => {
                                    error!("Completion stream serialization error: {}", error);
                                    send_stream_error(
                                        &tx,
                                        &format!("Serialization error: {}", error),
                                        "server_error",
                                        "internal_error",
                                    )
                                    .await;
                                    if let Some(lease) = lease.take() {
                                        let error = ProviderError::serialization(
                                            "router",
                                            format!("Serialization error: {}", error),
                                        );
                                        lease.finish_failure(&error);
                                    }
                                    settle_stream_spend_if_chargeable(
                                        pricing_service.as_ref(),
                                        &budget_limits,
                                        &key_manager,
                                        api_key_id,
                                        &served_provider,
                                        &served_model,
                                        final_usage.as_ref(),
                                        budget_reservation.take(),
                                        saw_upstream_output,
                                    )
                                    .await;
                                    return;
                                }
                            }
                        }
                        Err(error) => {
                            error!("Completion stream chunk error: {}", error);
                            let (error_type, error_code) =
                                super::chat::sse_error_classification(&error);
                            send_stream_error(&tx, &error.to_string(), error_type, error_code)
                                .await;
                            if let Some(lease) = lease.take() {
                                lease.finish_failure(&error);
                            }
                            settle_stream_spend_if_chargeable(
                                pricing_service.as_ref(),
                                &budget_limits,
                                &key_manager,
                                api_key_id,
                                &served_provider,
                                &served_model,
                                final_usage.as_ref(),
                                budget_reservation.take(),
                                saw_upstream_output,
                            )
                            .await;
                            return;
                        }
                    };

                    if tx.send(bytes).await.is_err() {
                        settle_stream_spend_if_chargeable(
                            pricing_service.as_ref(),
                            &budget_limits,
                            &key_manager,
                            api_key_id,
                            &served_provider,
                            &served_model,
                            final_usage.as_ref(),
                            budget_reservation.take(),
                            saw_upstream_output,
                        )
                        .await;
                        return;
                    }
                }

                let _ = tx.send(Event::default().data("[DONE]").to_bytes()).await;
                super::spend::record_finished_stream_spend_with_reservation_with_pricing(
                    pricing_service.as_ref(),
                    super::spend::StreamSpendSettlement {
                        budget_limits: &budget_limits,
                        key_manager: &key_manager,
                        api_key_id,
                        provider: &served_provider,
                        model: &served_model,
                        usage: final_usage.as_ref(),
                        saw_upstream_output,
                        budget_reservation: budget_reservation.take(),
                    },
                )
                .await;
                if let Some(lease) = lease.take() {
                    lease.finish_success(tokens_used);
                }
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
        Err(error) => {
            error!("Failed to create streaming completion response: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn settle_stream_spend(
    pricing_service: &crate::core::pricing_service::PricingService,
    budget_limits: &crate::core::budget::UnifiedBudgetLimits,
    key_manager: &crate::core::keys::KeyManager,
    api_key_id: Option<uuid::Uuid>,
    provider: &str,
    model: &str,
    usage: Option<&Usage>,
    budget_reservation: Option<crate::core::budget::UnifiedBudgetReservation>,
    saw_upstream_output: bool,
) {
    super::spend::record_finished_stream_spend_with_reservation_with_pricing(
        pricing_service,
        super::spend::StreamSpendSettlement {
            budget_limits,
            key_manager,
            api_key_id,
            provider,
            model,
            usage,
            saw_upstream_output,
            budget_reservation,
        },
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn settle_stream_spend_if_chargeable(
    pricing_service: &crate::core::pricing_service::PricingService,
    budget_limits: &crate::core::budget::UnifiedBudgetLimits,
    key_manager: &crate::core::keys::KeyManager,
    api_key_id: Option<uuid::Uuid>,
    provider: &str,
    model: &str,
    usage: Option<&Usage>,
    budget_reservation: Option<crate::core::budget::UnifiedBudgetReservation>,
    saw_upstream_output: bool,
) {
    if usage.is_some() || saw_upstream_output {
        settle_stream_spend(
            pricing_service,
            budget_limits,
            key_manager,
            api_key_id,
            provider,
            model,
            usage,
            budget_reservation,
            saw_upstream_output,
        )
        .await;
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

async fn send_stream_error(tx: &mpsc::Sender<Bytes>, message: &str, error_type: &str, code: &str) {
    let error_json = json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": code,
        }
    });
    let mut bytes = Event::default()
        .data(&error_json.to_string())
        .to_bytes()
        .to_vec();
    bytes.extend_from_slice(&Event::default().data("[DONE]").to_bytes());
    let _ = tx.send(Bytes::from(bytes)).await;
}
