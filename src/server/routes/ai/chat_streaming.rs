use std::time::Duration;

use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpResponse, Result as ActixResult};
use bytes::Bytes;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::core::models::openai::ChatCompletionRequest;
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::{context::SharedRequestContext, model::ProviderCapability};
use crate::server::state::AppState;

use super::super::budgeted::{ApiKeyBudgetPolicy, SettledStream, run_stream};
use super::super::callbacks::CallbackLifecycle;
use super::super::openai_errors;
use super::super::{spend, token_policy};

pub(super) async fn handle_streaming_chat_completion(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
) -> ActixResult<HttpResponse> {
    info!(
        "Handling streaming chat completion for model: {}",
        request.model
    );

    if let Err(error) = crate::server::guardrails::check_chat_input(state, request.as_ref()).await {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let requested_model = request.model.clone();
    let client_requested_usage = request
        .stream_options
        .as_ref()
        .and_then(|options| options.include_usage)
        .unwrap_or(false);
    let core_request = match super::build_core_chat_request_with_stream_usage(
        request.as_ref(),
        requested_model,
        true,
        Some(true),
    ) {
        Ok(req) => req,
        Err(e) => return Ok(openai_errors::gateway_error_response(&e)),
    };

    let requested_model = core_request.model.clone();
    let callback = CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        context.as_ref(),
    );
    let context_for_execution = Arc::clone(&context);
    let request_for_execution = Arc::clone(&request);
    let budgeted = state.budgeted.clone();
    let pricing_service = state.budgeted.pricing();
    let budget_limits = state.budgeted.budget_limits();
    let key_manager = state.budgeted.key_manager();
    let pricing_config = state.config().gateway.pricing.clone();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let callback_for_execution = callback.clone();
    match run_stream(
        state.unified_router.clone(),
        &requested_model,
        ProviderCapability::ChatCompletionStream,
        move |provider, selected_model, _selected_deployment_id| {
            let core_request = core_request.clone();
            let context = Arc::clone(&context_for_execution);
            let original_request = Arc::clone(&request_for_execution);
            let budgeted = budgeted.clone();
            let pricing_service = pricing_service.clone();
            let budget_limits = budget_limits.clone();
            let key_manager = key_manager.clone();
            let pricing_config = pricing_config.clone();
            let callback = callback_for_execution.clone();
            async move {
                let provider_name = provider.name().to_string();
                let (pricing_provider, pricing_model) = spend::pricing_identity_for_provider(
                    pricing_service.as_ref(),
                    &provider,
                    &selected_model,
                );
                let request_for_provider = token_policy::prepare_chat_request_for_provider(
                    context.api_key_max_tokens_per_request(),
                    &provider_name,
                    &selected_model,
                    core_request.clone(),
                )?;
                let request_for_budget =
                    spend::ChatCompletionBudgetRequest::from(original_request.as_ref())
                        .with_output_limits(
                            request_for_provider.max_tokens,
                            request_for_provider.max_completion_tokens,
                        );
                let provider_context = context.as_ref().clone();
                let reserve_pricing_service = pricing_service.clone();
                let reserve_pricing_config = pricing_config.clone();
                let reserve_pricing_provider = pricing_provider.clone();
                let reserve_pricing_model = pricing_model.clone();
                let callback_provider = provider_name.clone();
                let callback_model = selected_model.clone();
                let callback_pricing_provider = reserve_pricing_provider.clone();
                let callback_pricing_model = reserve_pricing_model.clone();
                let (stream, reservations) = budgeted
                    .for_selected_with_api_key_budget(
                        provider_name.clone(),
                        selected_model.clone(),
                        api_key_budget_id,
                        ApiKeyBudgetPolicy::FromProviderReservation,
                    )
                    .reserve_call(
                        |budget| {
                            spend::reserve_chat_completion_budget_with_split_pricing(
                                reserve_pricing_service.as_ref(),
                                &reserve_pricing_config,
                                budget.budget_limits(),
                                budget.provider(),
                                budget.model(),
                                &reserve_pricing_provider,
                                &reserve_pricing_model,
                                request_for_budget,
                            )
                        },
                        || {
                            callback.begin_provider_execution(
                                callback_provider,
                                callback_model,
                                callback_pricing_provider,
                                callback_pricing_model,
                            );
                            provider.chat_completion_stream(request_for_provider, provider_context)
                        },
                    )
                    .await?;
                let (budget_reservation, key_budget_reservation) = reservations.into_parts();
                let settlement = SettledStream {
                    pricing_service,
                    pricing_config,
                    budget_limits,
                    key_manager,
                    api_key_id,
                    provider: provider_name.clone(),
                    model: selected_model.clone(),
                    pricing_provider,
                    pricing_model,
                    budget_reservation,
                    key_budget_reservation,
                };
                Ok((stream, settlement))
            }
        },
    )
    .await
    {
        Ok(((mut stream, mut settlement), lease)) => {
            let (tx, rx) = mpsc::channel::<Bytes>(8);
            let idle_timeout_secs = state.config.load().gateway.server.stream_idle_timeout;
            let guardrails = Arc::clone(&state.guardrails);

            tokio::spawn(async move {
                let mut lease = Some(lease);
                let mut output_guardrail =
                    super::super::stream_output_guardrail::StreamOutputGuardrail::new(guardrails);
                let mut tokens_used = 0_u64;
                let mut final_usage = None;
                let mut saw_upstream_output = false;
                macro_rules! settle_after_upstream_output {
                    () => {
                        if final_usage.is_some() || saw_upstream_output {
                            settlement.record_disconnect(final_usage.as_ref()).await;
                        }
                    };
                }
                macro_rules! return_after_guardrail_error {
                    ($error:expr) => {{
                        let guardrail_error = $error;
                        let error_bytes = super::format_sse_error(
                            guardrail_error.message(),
                            guardrail_error.error_type(),
                            guardrail_error.code(),
                        );
                        if tx.send(error_bytes).await.is_err() {
                            info!("Client disconnected before guardrail error could be sent");
                        }
                        drop(lease.take());
                        callback.fail(guardrail_error.message(), "guardrail_output");
                        settle_after_upstream_output!();
                        return;
                    }};
                }

                loop {
                    let chunk_result = if idle_timeout_secs == 0 {
                        tokio::select! {
                            biased;
                            _ = tx.closed() => {
                                info!("Client disconnected while waiting for stream output");
                                callback.fail("client disconnected", "client_disconnect");
                                settle_after_upstream_output!();
                                return;
                            }
                            result = stream.next() => result,
                        }
                    } else {
                        let timeout_dur = Duration::from_secs(idle_timeout_secs);
                        let timed_result = tokio::select! {
                            biased;
                            _ = tx.closed() => {
                                info!("Client disconnected while waiting for stream output");
                                callback.fail("client disconnected", "client_disconnect");
                                settle_after_upstream_output!();
                                return;
                            }
                            result = tokio::time::timeout(timeout_dur, stream.next()) => result,
                        };
                        match timed_result {
                            Ok(result) => result,
                            Err(_) => {
                                warn!(
                                    "SSE stream idle timeout after {}s, closing connection",
                                    idle_timeout_secs
                                );
                                let error_bytes = super::format_sse_error(
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
                                if let Some(lease) = lease.take() {
                                    let error = ProviderError::timeout(
                                        "router",
                                        format!("stream idle timeout after {}s", idle_timeout_secs),
                                    );
                                    lease.finish_failure(&error);
                                }
                                callback.fail(
                                    format!("stream idle timeout after {}s", idle_timeout_secs),
                                    "timeout",
                                );
                                settle_after_upstream_output!();
                                return;
                            }
                        }
                    };

                    let Some(chunk_result) = chunk_result else {
                        break;
                    };

                    let (output_deltas, bytes) = match chunk_result {
                        Ok(chunk) => {
                            let has_candidate_output =
                                spend::stream_chunk_has_candidate_output(&chunk);
                            if let Some(usage) = &chunk.usage {
                                final_usage = Some(usage.clone());
                            }
                            tokens_used = final_usage
                                .as_ref()
                                .map(|usage| u64::from(usage.total_tokens))
                                .unwrap_or(0);
                            if chunk.choices.is_empty() && chunk.usage.is_none() {
                                continue;
                            }
                            saw_upstream_output |= has_candidate_output;
                            let mut chat_chunk = match super::convert_core_chunk_to_streaming(chunk)
                            {
                                Ok(chat_chunk) => chat_chunk,
                                Err(e) => {
                                    error!("Stream chunk conversion error: {}", e);
                                    let (error_type, error_code) =
                                        super::sse_error_classification(&e);
                                    let error_bytes = super::format_sse_error(
                                        &e.to_string(),
                                        error_type,
                                        error_code,
                                    );
                                    if tx.send(error_bytes).await.is_err() {
                                        info!(
                                            "Client disconnected before conversion error could be sent"
                                        );
                                    }
                                    if let Some(lease) = lease.take() {
                                        lease.finish_failure(&e);
                                    }
                                    callback.fail(e.to_string(), "conversion_error");
                                    settle_after_upstream_output!();
                                    return;
                                }
                            };
                            if !client_requested_usage {
                                chat_chunk.usage = None;
                                if chat_chunk.choices.is_empty() {
                                    continue;
                                }
                            }
                            let output_deltas = chat_chunk
                                .choices
                                .iter()
                                .filter_map(|choice| {
                                    choice
                                        .delta
                                        .content
                                        .as_ref()
                                        .map(|text| (choice.index, text.clone()))
                                })
                                .collect::<Vec<_>>();
                            let bytes = match serde_json::to_string(&chat_chunk) {
                                Ok(json) => {
                                    let event = Event::default().data(&json);
                                    event.to_bytes()
                                }
                                Err(e) => {
                                    error!("Stream serialization error: {}", e);
                                    let error_bytes = super::format_sse_error(
                                        &format!("Serialization error: {}", e),
                                        "server_error",
                                        "internal_error",
                                    );
                                    if tx.send(error_bytes).await.is_err() {
                                        info!(
                                            "Client disconnected before error event could be sent"
                                        );
                                    }
                                    if let Some(lease) = lease.take() {
                                        let error = ProviderError::serialization(
                                            "router",
                                            format!("Serialization error: {}", e),
                                        );
                                        lease.finish_failure(&error);
                                    }
                                    callback.fail(
                                        format!("Serialization error: {}", e),
                                        "serialization_error",
                                    );
                                    settle_after_upstream_output!();
                                    return;
                                }
                            };
                            (output_deltas, bytes)
                        }
                        Err(e) => {
                            error!("Stream chunk error: {}", e);
                            let (error_type, error_code) = super::sse_error_classification(&e);
                            let error_bytes =
                                super::format_sse_error(&e.to_string(), error_type, error_code);
                            if tx.send(error_bytes).await.is_err() {
                                info!("Client disconnected before error event could be sent");
                            }
                            if let Some(lease) = lease.take() {
                                lease.finish_failure(&e);
                            }
                            callback.fail(e.to_string(), "provider_error");
                            settle_after_upstream_output!();
                            return;
                        }
                    };

                    let pending = match output_guardrail
                        .push_many_until_closed(&tx, output_deltas, bytes)
                        .await
                    {
                        Ok(Some(pending)) => pending,
                        Ok(None) => {
                            callback.fail("client disconnected", "client_disconnect");
                            settle_after_upstream_output!();
                            return;
                        }
                        Err(error) => return_after_guardrail_error!(error),
                    };
                    for bytes in pending {
                        if tx.send(bytes).await.is_err() {
                            info!("Client disconnected during streaming, cancelling upstream");
                            callback.fail("client disconnected", "client_disconnect");
                            settle_after_upstream_output!();
                            return;
                        }
                    }
                }

                let pending = match output_guardrail.finish_until_closed(&tx).await {
                    Ok(Some(pending)) => pending,
                    Ok(None) => {
                        callback.fail("client disconnected", "client_disconnect");
                        settle_after_upstream_output!();
                        return;
                    }
                    Err(error) => return_after_guardrail_error!(error),
                };
                for bytes in pending {
                    if tx.send(bytes).await.is_err() {
                        callback.fail("client disconnected", "client_disconnect");
                        settle_after_upstream_output!();
                        return;
                    }
                }

                let done_event = Event::default().data("[DONE]");
                if tx.send(done_event.to_bytes()).await.is_err() {
                    info!("Client disconnected before [DONE] event could be sent");
                    callback.fail("client disconnected", "client_disconnect");
                    settle_after_upstream_output!();
                    return;
                }
                settlement
                    .record_completion(final_usage.as_ref(), saw_upstream_output)
                    .await;
                callback.complete_usage(final_usage.as_ref(), "success");
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
        Err(e) => {
            error!("Failed to create streaming response: {}", e);
            callback.fail(e.to_string(), "provider_error");
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}
