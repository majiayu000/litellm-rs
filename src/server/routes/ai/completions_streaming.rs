use std::time::Duration;

use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpResponse, Result as ActixResult};
use bytes::Bytes;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::{context::SharedRequestContext, model::ProviderCapability};
use crate::server::state::AppState;

use super::super::budgeted::{ApiKeyBudgetPolicy, SettledStream, run_stream};
use super::super::callbacks::CallbackLifecycle;
use super::super::{chat, openai_errors, spend, token_policy};
use super::completions_sse::send_stream_error;
use super::{CompletionAdapterRequest, chunk_has_text_delta, completion_chunk_from_core};

pub(super) async fn handle_streaming_completion(
    state: &AppState,
    adapter_request: CompletionAdapterRequest,
    context: SharedRequestContext,
) -> ActixResult<HttpResponse> {
    info!(
        "Handling streaming text completion for model: {}",
        adapter_request.chat_request.model
    );

    let request = Arc::new(adapter_request.chat_request);
    if let Err(error) = crate::server::guardrails::check_chat_input(state, request.as_ref()).await {
        return Ok(openai_errors::gateway_error_response(&error));
    }
    let requested_model = request.model.clone();

    let core_request = match chat::build_core_chat_request_with_stream_usage(
        request.as_ref(),
        requested_model.clone(),
        true,
        Some(true),
    ) {
        Ok(request) => request,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

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
                let request_pricing = spend::request_pricing_for_provider(
                    &pricing_service,
                    &provider,
                    &selected_model,
                )?;
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
                let reserve_pricing_config = pricing_config.clone();
                let reserve_request_pricing = request_pricing.clone();
                let callback_provider = provider_name.clone();
                let callback_model = selected_model.clone();
                let callback_request_pricing = request_pricing.clone();
                let (stream, reservations) = budgeted
                    .for_selected_with_api_key_budget(
                        provider_name.clone(),
                        selected_model.clone(),
                        api_key_budget_id,
                        ApiKeyBudgetPolicy::FromProviderReservation,
                    )
                    .reserve_call(
                        |budget| {
                            spend::reserve_chat_completion_budget_with_request_pricing(
                                &reserve_request_pricing,
                                &reserve_pricing_config,
                                budget.budget_limits(),
                                budget.provider(),
                                budget.model(),
                                request_for_budget,
                            )
                        },
                        || {
                            callback.begin_provider_execution_with_pricing(
                                callback_provider,
                                callback_model,
                                callback_request_pricing,
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
                    request_pricing,
                    budget_reservation,
                    key_budget_reservation,
                };
                Ok((stream, settlement))
            }
        },
    )
    .await
    {
        Ok(((mut stream, settlement), lease)) => {
            let (tx, rx) = mpsc::channel::<Bytes>(8);
            let idle_timeout_secs = state.config.load().gateway.server.stream_idle_timeout;
            let include_usage = adapter_request.include_usage;
            let mut echo_prefix = adapter_request.echo.then_some(adapter_request.prompt);
            let guardrails = Arc::clone(&state.guardrails);

            tokio::spawn(async move {
                let mut lease = Some(lease);
                let mut settlement = settlement.into_abort_safe();
                let mut output_guardrail =
                    super::super::stream_output_guardrail::StreamOutputGuardrail::new(guardrails);
                let mut tokens_used = 0_u64;
                let mut final_usage = None;
                let mut saw_upstream_output = false;
                macro_rules! settle_if_chargeable {
                    () => {
                        if final_usage.is_some() || saw_upstream_output {
                            settlement.record_disconnect(final_usage.as_ref()).await;
                        }
                    };
                }
                macro_rules! return_after_guardrail_error {
                    ($error:expr) => {{
                        let guardrail_error = $error;
                        send_stream_error(
                            &tx,
                            guardrail_error.message(),
                            guardrail_error.error_type(),
                            guardrail_error.code(),
                        )
                        .await;
                        drop(lease.take());
                        callback.fail(guardrail_error.message(), "guardrail_output");
                        settle_if_chargeable!();
                        return;
                    }};
                }
                macro_rules! flush_guardrail {
                    () => {
                        match output_guardrail.flush_to_until_closed(&tx).await {
                            Ok(Some(())) => {}
                            Ok(None) => {
                                callback.fail("client disconnected", "client_disconnect");
                                settle_if_chargeable!();
                                return;
                            }
                            Err(error) => return_after_guardrail_error!(error),
                        }
                    };
                }

                loop {
                    let chunk_result = if idle_timeout_secs == 0 {
                        tokio::select! {
                            biased;
                            _ = tx.closed() => {
                                callback.fail("client disconnected", "client_disconnect");
                                settle_if_chargeable!();
                                return;
                            }
                            result = stream.next() => result,
                        }
                    } else {
                        let timed_result = tokio::select! {
                            biased;
                            _ = tx.closed() => {
                                callback.fail("client disconnected", "client_disconnect");
                                settle_if_chargeable!();
                                return;
                            }
                            result = tokio::time::timeout(
                                Duration::from_secs(idle_timeout_secs),
                                stream.next(),
                            ) => result,
                        };
                        match timed_result {
                            Ok(result) => result,
                            Err(_) => {
                                flush_guardrail!();
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
                                callback.fail(
                                    format!("stream idle timeout after {}s", idle_timeout_secs),
                                    "timeout",
                                );
                                settle_if_chargeable!();
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
                            settlement.observe(final_usage.as_ref(), saw_upstream_output);
                            let prefix_for_chunk = if chunk_has_text_delta(&chunk) {
                                echo_prefix.take()
                            } else {
                                None
                            };
                            let output_deltas = chunk
                                .choices
                                .iter()
                                .map(|choice| {
                                    (
                                        choice.index,
                                        choice.delta.content.clone().unwrap_or_default(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            let completion_chunk = completion_chunk_from_core(
                                chunk,
                                prefix_for_chunk.as_deref(),
                                include_usage,
                            );
                            if !include_usage && completion_chunk.choices.is_empty() {
                                continue;
                            }
                            let bytes = match serde_json::to_string(&completion_chunk) {
                                Ok(json) => Event::default().data(&json).to_bytes(),
                                Err(error) => {
                                    flush_guardrail!();
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
                                    callback.fail(
                                        format!("Serialization error: {}", error),
                                        "serialization_error",
                                    );
                                    settle_if_chargeable!();
                                    return;
                                }
                            };
                            (output_deltas, bytes)
                        }
                        Err(error) => {
                            flush_guardrail!();
                            error!("Completion stream chunk error: {}", error);
                            let (error_type, error_code) = chat::sse_error_classification(&error);
                            send_stream_error(&tx, &error.to_string(), error_type, error_code)
                                .await;
                            if let Some(lease) = lease.take() {
                                lease.finish_failure(&error);
                            }
                            callback.fail(error.to_string(), "provider_error");
                            settle_if_chargeable!();
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
                            settle_if_chargeable!();
                            return;
                        }
                        Err(error) => return_after_guardrail_error!(error),
                    };
                    for bytes in pending {
                        if tx.send(bytes).await.is_err() {
                            callback.fail("client disconnected", "client_disconnect");
                            settle_if_chargeable!();
                            return;
                        }
                    }
                }

                flush_guardrail!();

                if tx
                    .send(Event::default().data("[DONE]").to_bytes())
                    .await
                    .is_err()
                {
                    callback.fail("client disconnected", "client_disconnect");
                    settle_if_chargeable!();
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
        Err(error) => {
            error!("Failed to create streaming completion response: {}", error);
            callback.fail(error.to_string(), "provider_error");
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}
