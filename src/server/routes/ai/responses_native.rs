//! Native Responses creation through the shared deployment and budget pipeline.
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use futures::StreamExt;
use serde_json::Value;

use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::models::openai::{
    ChatCompletionRequest, ChatMessage, ContentPart, ImageUrl, MessageContent, MessageRole,
};
use crate::core::providers::ProviderError;
use crate::core::request_ledger::SharedRequestLedgerFacts;
use crate::core::types::context::RequestContext;
use crate::core::types::model::ProviderCapability;
use crate::core::types::responses::Usage;
use crate::server::guardrails::{self, GuardrailDecisionSink};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::{budgeted, openai_errors, spend};
#[path = "responses_native_background.rs"]
pub(super) mod background;
#[path = "responses_native_lifecycle.rs"]
pub(super) mod lifecycle;
#[path = "responses_native_request.rs"]
mod request;
#[path = "responses_native_stream.rs"]
mod stream;
use lifecycle::NativeResponseStorage;

const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

pub async fn create_response(
    state: web::Data<AppState>,
    req: HttpRequest,
    payload: web::Json<Value>,
) -> ActixResult<HttpResponse> {
    let body = payload.into_inner();
    let router = state.unified_router();
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let model = router.resolve_model_name(model);
    let native = router
        .get_deployments_for_model(&model)
        .iter()
        .filter_map(|id| router.get_deployment(id))
        .any(|deployment| {
            deployment
                .provider
                .capabilities()
                .contains(&ProviderCapability::Responses)
        });
    if !native {
        return match serde_json::from_value(body) {
            Ok(body) => super::responses::create_response(state, req, web::Json(body)).await,
            Err(_) => Ok(openai_errors::validation_error("Invalid Responses request")),
        };
    }
    match create_native(state.get_ref(), &req, body, false).await {
        Ok(response) => Ok(response),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

pub async fn compact_response(
    state: web::Data<AppState>,
    req: HttpRequest,
    payload: web::Json<Value>,
) -> ActixResult<HttpResponse> {
    match create_native(state.get_ref(), &req, payload.into_inner(), true).await {
        Ok(response) => Ok(response),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

struct NativeCall {
    callback: super::callbacks::CallbackLifecycle,
    response: reqwest::Response,
    provider: String,
    model: String,
    deployment: String,
    pricing: spend::RequestPricing,
    file_search_calls: Option<u32>,
    reservation: Option<UnifiedBudgetReservation>,
    key_reservation: Option<BudgetReservation>,
    storage: Option<NativeResponseStorage>,
    started: tokio::time::Instant,
}

async fn create_native(
    state: &AppState,
    req: &HttpRequest,
    mut body: Value,
    compact: bool,
) -> Result<HttpResponse, GatewayError> {
    let mut context = super::context::get_request_context(req)
        .map_err(|_| GatewayError::Auth("Unauthorized".into()))?;
    let config = state.config();
    let auth = &config.gateway.auth;
    if (auth.enable_jwt || auth.enable_api_key || !auth.allow_anonymous)
        && !super::context::check_permission(
            super::context::get_authenticated_user(req).as_ref(),
            super::context::get_authenticated_api_key(req).as_ref(),
            "chat",
        )
    {
        return Err(GatewayError::Auth("Unauthorized".into()));
    }
    super::token_policy::attach_api_key_token_limit(req, &mut context)?;
    let owner = super::responses::response_owner(&context);
    if compact {
        for field in ["stream", "background", "store", "max_output_tokens"] {
            if body.get(field).is_some() {
                return Err(GatewayError::validation(format!(
                    "{field} is not supported by Responses compaction"
                )));
            }
        }
        if context.api_key_max_tokens_per_request().is_some() {
            return Err(GatewayError::Forbidden(
                "Compaction cannot enforce this key's output token limit".into(),
            ));
        }
    }
    let store = match body.get("store") {
        None | Some(Value::Null) => !compact,
        Some(Value::Bool(value)) => *value,
        _ => return Err(GatewayError::validation("store must be a boolean")),
    };
    let background = match body.get("background") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(GatewayError::validation("background must be a boolean")),
    };
    if background && context.api_key_budget_id().is_some() {
        return Err(GatewayError::validation(
            "Background Responses cannot use an in-process API key budget; persistent key usage remains supported",
        ));
    }
    if background && !state.config().gateway.storage.database.enabled {
        return Err(GatewayError::validation(
            "Background Responses require an enabled shared SQL database",
        ));
    }
    if (store || background) && owner.is_none() {
        return Err(GatewayError::validation(
            "Stored or background Responses require authentication; anonymous requests must use store=false and background=false",
        ));
    }
    let previous =
        lifecycle::previous_response(&state.storage.database, &body, owner.as_ref()).await?;
    let retained_prompt_tokens = previous.as_ref().map_or(0, |(_, tokens)| *tokens);
    let billing_scope = request::validate(&body)?;
    let needs_input_count = billing_scope.count_input;
    let file_search_calls = billing_scope.file_search_calls;
    if compact && file_search_calls.is_some() {
        return Err(GatewayError::validation(
            "file_search is not supported by compaction",
        ));
    }
    let requested_model = body
        .get("model")
        .and_then(Value::as_str)
        .filter(|model| !model.trim().is_empty())
        .ok_or_else(|| GatewayError::validation("model must not be empty"))?
        .to_string();
    let max_tokens = match body.get("max_output_tokens") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    GatewayError::validation("max_output_tokens must be a positive integer")
                })?,
        ),
    };
    super::context::enforce_api_key_model_and_token_limits(req, &requested_model, max_tokens)?;
    if max_tokens.is_none()
        && let Some(limit) = context.api_key_max_tokens_per_request()
    {
        body["max_output_tokens"] = limit.into();
    }
    let streaming = match body.get("stream") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(GatewayError::validation("stream must be a boolean")),
    };
    if streaming {
        guardrails::reject_unsupported_streaming_mask(state)?;
    }
    let sink = GuardrailDecisionSink::from_state(state, Some(&requested_model), None, None);
    body =
        guardrails::apply_native_responses(state.guardrails().as_ref(), body, false, &sink).await?;
    let retained_input = lifecycle::retained_input(
        state,
        &body,
        previous.as_ref().map(|(record, _)| record),
        &sink,
    )
    .await?;
    let model = state.unified_router().resolve_model_name(&requested_model);
    let projected = budget_request(&body, &model);
    let estimated_prompt_tokens = spend::try_estimate_chat_prompt_tokens(
        &crate::utils::ai::counter::token_counter::TokenizerIdentity::approximate(
            "responses",
            &model,
        ),
        &projected.messages,
        None,
        None,
        None,
        None,
    )?;
    let estimated_tokens = u64::from(estimated_prompt_tokens)
        .saturating_add(u64::from(retained_prompt_tokens))
        .saturating_add(u64::from(projected.max_tokens.unwrap_or(0)));
    let callback = super::callbacks::CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        &context,
    );
    let (mut call, mut lease) = super::execution::execute_stream_with_selected_deployment_matching_with_idempotency(
        state.unified_router(),
        &model,
        ProviderCapability::Responses,
        |deployment| {
            (!(compact || file_search_calls.is_some()) || matches!(&deployment.provider, crate::core::providers::Provider::OpenAI(_)))
                && previous.as_ref().is_none_or(|(record, _)| {
                record.deployment_id.as_deref() == Some(deployment.id.as_str())
                    && deployment.provider.native_response_binding() == record.deployment_binding
            })
        },
        crate::core::router::retry_policy::RequestIdempotency::NonIdempotent,
        estimated_tokens,
        {
            let context = context.clone();
            let callback = callback.clone();
            let owner = owner.clone();
            move |provider, model, deployment| {
                let mut body = body.clone();
                let retained_input = retained_input.clone();
                let context = context.clone();
                let callback = callback.clone();
                let owner = owner.clone();
                async move {
                    body["model"] = model.clone().into();
                    let provider_name = provider.name().to_string();
                    let pricing = spend::request_pricing_for_provider(
                        &state.budgeted.pricing(),
                        &provider,
                        &model,
                        ProviderCapability::Responses,
                    )?;
                    // This projection is only for the token reservation. The native wire body
                    // never passes through the chat transformer, including tools and reasoning.
                    let mut budget_request = budget_request(&body, &model);
                    if compact {
                        budget_request.max_tokens = Some(
                            pricing.model_info()
                                .and_then(|info| info.max_output_tokens)
                                .filter(|limit| *limit > 0)
                                .ok_or_else(|| ProviderError::invalid_request(
                                    "openai",
                                    "Compaction requires a verified model output bound for budget reservation",
                                ))?,
                        );
                    }
                    let mut storage = if store || background {
                        let binding = provider.native_response_binding().ok_or_else(|| {
                            ProviderError::not_supported("responses", "Stored native responses")
                        })?;
                        let owner = owner.as_ref().ok_or_else(|| {
                            ProviderError::invalid_request(
                                "responses",
                                "Stored responses require an authenticated owner",
                            )
                        })?;
                        Some(NativeResponseStorage::new(
                            owner.0.clone(),
                            &retained_input,
                            deployment.clone(),
                            binding,
                        ))
                    } else {
                        None
                    };
                    let counted_input = if needs_input_count {
                        Some(
                            provider
                                .native_response_input_tokens(request::count_body(&body))
                                .await?,
                        )
                    } else {
                        None
                    };
                    let limits = state.budgeted.budget_limits();
                    let started = tokio::time::Instant::now();
                    let (reservations, _) = state
                        .budgeted
                        .for_selected_with_api_key_budget(
                            provider_name.clone(),
                            model.clone(),
                            context.api_key_budget_id(),
                            budgeted::ApiKeyBudgetPolicy::FromProviderReservation,
                        )
                        .reserve_for_call(async |_| {
                            if let Some(calls) = file_search_calls {
                                reserve_file_search_budget(
                                    &pricing, &limits, &provider_name, &model,
                                    counted_input, budget_request.max_tokens, calls,
                                ).await
                            } else if let Some(input_tokens) = counted_input {
                                spend::reserve_completion_budget_with_counted_input(
                                    &pricing,
                                    &state.config().gateway.pricing,
                                    &limits,
                                    &provider_name,
                                    &model,
                                    input_tokens,
                                    budget_request.max_tokens,
                                ).await
                            } else {
                                spend::reserve_chat_completion_budget_with_request_pricing(
                                    &pricing,
                                    &state.config().gateway.pricing,
                                    &limits,
                                    &provider_name,
                                    &model,
                                    spend::ChatCompletionBudgetRequest::from(&budget_request)
                                        .with_retained_prompt_tokens(retained_prompt_tokens),
                                ).await
                            }
                        }).await?;
                    let (mut reservation, key_reservation) = reservations.into_parts();
                    if background {
                        super::responses_settlement::prepare(
                            state,
                            &context,
                            storage.as_mut().expect("background storage checked above"),
                            &provider_name,
                            &model,
                            &pricing,
                            reservation.as_ref(),
                        )
                        .await
                        .map_err(|error| {
                            ProviderError::configuration("responses", error.to_string())
                        })?;
                        if let Some(reservation) = reservation.take() {
                            reservation.detach_response();
                        }
                    }
                    callback.begin_provider_execution_with_pricing(
                        &provider_name,
                        &model,
                        pricing.clone(),
                    );
                    let mut generation_attempted = false;
                    let response = if compact {
                        generation_attempted = true;
                        provider.compact_response(body).await
                    } else {
                        provider.native_response(body, &mut generation_attempted).await
                    };
                    let response = match response {
                        Ok(response) => response,
                        Err(error) => {
                            // An accepted POST can lose its response headers. Foreground
                            // calls have no durable owner yet, so settle unknown usage
                            // before dropping their reservations. Background obligations
                            // already belong to the durable recovery path above.
                            if !background && generation_attempted && matches!(
                                error,
                                ProviderError::Network { .. } | ProviderError::Timeout { .. }
                            ) {
                                settle(
                                    state, &context, &provider_name, &model, pricing,
                                    None, None, reservation, key_reservation,
                                    crate::core::request_ledger::current_facts(),
                                ).await;
                            }
                            return Err(error);
                        }
                    };
                    Ok(NativeCall {
                        callback,
                        response,
                        provider: provider_name,
                        model,
                        deployment,
                        pricing,
                        file_search_calls,
                        reservation,
                        key_reservation,
                        storage,
                        started,
                    })
                }
            }
        },
    )
    .await
    .inspect_err(|error| {
        callback.fail(error.to_string(), "provider_error");
    })?;
    if streaming {
        return Ok(stream::response(state.clone(), context, call, lease));
    }
    if background {
        let value = lifecycle::read_json(&mut call.response).await;
        return background::response(
            state.clone(),
            context,
            call,
            lease,
            value,
            false,
            crate::core::request_ledger::current_facts(),
        )
        .await;
    }
    let NativeCall {
        callback,
        response,
        provider,
        model,
        deployment,
        pricing,
        file_search_calls,
        reservation,
        key_reservation,
        mut storage,
        started: _,
    } = call;
    let mut body_stream = response.bytes_stream();
    let mut bytes = Vec::new();
    let mut failure = None;
    while let Some(chunk) = body_stream.next().await {
        match chunk {
            Ok(chunk) if bytes.len().saturating_add(chunk.len()) <= MAX_RESPONSE_BYTES => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(_) => {
                failure = Some(ProviderError::response_parsing(
                    "responses",
                    "Responses body exceeds size limit",
                ));
                break;
            }
            Err(_) => {
                failure = Some(ProviderError::network(
                    "responses",
                    "Responses body interrupted",
                ));
                break;
            }
        }
    }
    let value = serde_json::from_slice::<Value>(&bytes);
    let usage = value.as_ref().ok().and_then(response_usage);
    let pricing_usage = value
        .as_ref()
        .ok()
        .zip(usage.as_ref())
        .and_then(|(value, usage)| request::pricing_usage(value, usage, file_search_calls));
    let value =
        value.map_err(|_| ProviderError::response_parsing("responses", "Invalid Responses JSON"));
    let value = match failure {
        Some(error) => Err(error),
        None => value,
    };
    let value = value.and_then(|value| {
        if compact
            && (value.get("error").is_some_and(|error| !error.is_null())
                || value.get("object").and_then(Value::as_str) != Some("response.compaction")
                || value
                    .get("id")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                || value.get("output").and_then(Value::as_array).is_none()
                || usage.is_none())
        {
            Err(ProviderError::response_parsing(
                "responses",
                "Invalid compaction response",
            ))
        } else {
            Ok(value)
        }
    });
    let tokens_used = usage
        .as_ref()
        .map_or(0, |usage| u64::from(usage.total_tokens));
    let terminal_error = value.as_ref().err().cloned().or_else(|| {
        value
            .as_ref()
            .ok()
            .filter(|value| {
                value.get("error").is_some_and(|error| !error.is_null())
                    || value.get("status").and_then(Value::as_str) == Some("failed")
            })
            .map(|_| ProviderError::api_error("responses", 502, "Upstream response failed"))
    });
    if usage.is_none() {
        lease.finish_unknown(true, terminal_error.as_ref()).await;
    }
    lease
        .settle_terminal(
            tokens_used,
            terminal_error.as_ref(),
            settle(
                state,
                &context,
                &provider,
                &model,
                pricing,
                usage.as_ref(),
                pricing_usage.clone(),
                reservation,
                key_reservation,
                crate::core::request_ledger::current_facts(),
            ),
        )
        .await;
    let value = match value {
        Ok(value) => value,
        Err(error) => {
            callback.fail(error.to_string(), "provider_error");
            lease.finish_failure_with_tokens(&error, tokens_used).await;
            return Err(error.into());
        }
    };
    if value.get("error").is_some_and(|error| !error.is_null())
        || value.get("status").and_then(Value::as_str) == Some("failed")
    {
        callback.fail("Upstream response failed", "provider_error");
        lease
            .finish_failure_with_tokens(
                &ProviderError::api_error("responses", 502, "Upstream response failed"),
                tokens_used,
            )
            .await;
    } else {
        lease
            .finish_success(
                usage
                    .as_ref()
                    .map_or(0, |usage| u64::from(usage.total_tokens)),
            )
            .await;
    }
    let sink =
        GuardrailDecisionSink::from_state(state, Some(&model), Some(&provider), Some(&deployment));
    let value = guardrails::apply_native_responses(state.guardrails().as_ref(), value, true, &sink)
        .await
        .inspect_err(|error| {
            callback.fail(error.to_string(), "guardrail_error");
        })?;
    if let Some(storage) = storage.as_mut() {
        storage
            .save(&state.storage.database, &value)
            .await
            .inspect_err(|error| {
                callback.fail(error.to_string(), "storage_error");
            })?;
    }
    callback.complete_pricing_usage(usage.as_ref(), pricing_usage.as_ref(), "success");
    Ok(HttpResponse::Ok().json(value))
}

#[allow(clippy::too_many_arguments)]
async fn reserve_file_search_budget(
    pricing: &spend::RequestPricing,
    limits: &crate::core::budget::UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    counted_input: Option<u32>,
    max_output: Option<u32>,
    calls: u32,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let context_tokens = pricing
        .model_info()
        .and_then(|info| info.max_input_tokens)
        .filter(|limit| *limit > 0)
        .ok_or_else(|| {
            ProviderError::invalid_request(
                "responses",
                "file_search requires verified model context and pricing bounds",
            )
        })?;
    let input = context_tokens
        .checked_mul(calls)
        .and_then(|tokens| counted_input?.checked_add(tokens))
        .ok_or_else(|| {
            ProviderError::invalid_request(
                "responses",
                "file_search input reservation exceeds supported range",
            )
        })?;
    let output = max_output
        .or_else(|| pricing.model_info().and_then(|info| info.max_output_tokens))
        .filter(|limit| *limit > 0)
        .ok_or_else(|| {
            ProviderError::invalid_request(
                "responses",
                "file_search requires a verified model output bound or max_output_tokens",
            )
        })?;
    let estimate = pricing
        .estimate_completion(input, Some(output))
        .map_err(|error| ProviderError::configuration("responses", error.to_string()))?;
    let mut tool_usage = crate::core::pricing_service::PricingUsage::new(0, 0);
    tool_usage.file_search_requests = Some(calls);
    let tool_cost = pricing
        .calculate_usage(&tool_usage)
        .map_err(|error| ProviderError::configuration("responses", error.to_string()))?
        .tool_cost;
    limits
        .reserve_spend_async(provider, model, estimate.max_cost + tool_cost)
        .await
        .map(Some)
        .map_err(|error| spend::reservation_error_to_provider_error(error, provider, model))
}

fn budget_request(body: &Value, model: &str) -> ChatCompletionRequest {
    let mut parts = Vec::new();
    let mut text_body = body.clone();
    budget_image_parts(&mut text_body, &mut parts);
    // Preserve tool schemas, instructions and other native text, without counting
    // an image's base64 payload both as text tokens and as image overhead.
    parts.insert(
        0,
        ContentPart::Text {
            text: text_body.to_string(),
        },
    );
    ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Parts(parts)),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        max_tokens: body
            .get("max_output_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        ..Default::default()
    }
}

fn budget_image_parts(value: &mut Value, parts: &mut Vec<ContentPart>) {
    match value {
        Value::Array(values) => {
            for value in values {
                budget_image_parts(value, parts);
            }
        }
        Value::Object(values) => {
            if values.get("type").and_then(Value::as_str) == Some("input_image") {
                parts.push(ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: values
                            .get("image_url")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        detail: values
                            .get("detail")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    },
                });
                *value = Value::Null;
                return;
            }
            for value in values.values_mut() {
                budget_image_parts(value, parts);
            }
        }
        _ => {}
    }
}

pub(super) fn response_usage(value: &Value) -> Option<Usage> {
    let usage = value.get("usage")?;
    let input = usage.get("input_tokens")?.as_u64()?;
    let output = usage.get("output_tokens")?.as_u64()?;
    let total = usage.get("total_tokens")?.as_u64()?;
    let cached = optional_tokens(usage.pointer("/input_tokens_details/cached_tokens"))?;
    let written = optional_tokens(usage.pointer("/input_tokens_details/cache_write_tokens"))?;
    let reasoning = optional_tokens(usage.pointer("/output_tokens_details/reasoning_tokens"))?;
    if reasoning > output || cached.checked_add(written)? > input {
        return None;
    }
    let mut normalized = crate::core::providers::shared::strict_usage(
        &[input],
        &[output],
        Some((total, &[input, output])),
        Some((cached, input)),
    )?;
    normalized
        .prompt_tokens_details
        .as_mut()?
        .cache_creation_tokens = Some(u32::try_from(written).ok()?);
    normalized.completion_tokens_details =
        Some(crate::core::types::responses::CompletionTokensDetails {
            reasoning_tokens: Some(u32::try_from(reasoning).unwrap_or(u32::MAX)),
            audio_tokens: None,
        });
    Some(normalized)
}

fn optional_tokens(value: Option<&Value>) -> Option<u64> {
    match value {
        None | Some(Value::Null) => Some(0),
        Some(value) => value.as_u64(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn settle(
    state: &AppState,
    context: &RequestContext,
    provider: &str,
    model: &str,
    pricing: spend::RequestPricing,
    usage: Option<&Usage>,
    pricing_usage: Option<crate::core::pricing_service::PricingUsage>,
    reservation: Option<UnifiedBudgetReservation>,
    key_reservation: Option<BudgetReservation>,
    facts: Option<SharedRequestLedgerFacts>,
) {
    let captured_facts = facts.clone();
    let work = async move {
        let budgeted = &state.budgeted;
        let limits = budgeted.budget_limits();
        let keys = budgeted.key_manager();
        if pricing_usage.is_none() {
            spend::capture_ledger_settlement(facts.as_ref(), provider, model, usage, None);
            // Preserve the budget upper bound without presenting it as an actual bill.
            if let Some(reservation) = key_reservation {
                let reserved = reservation.reserved_amount();
                spend::settle_api_key_budget_reservation(
                    Some(reservation),
                    reserved,
                    "Responses usage unknown",
                );
            }
            if let Some(usage) = usage {
                super::execution::completion::observe_usage(u64::from(usage.total_tokens));
            }
            let budget_settlement = async {
                if let Some(reservation) = reservation {
                    let reserved = reservation.reserved_amount();
                    if let Err(error) =
                        super::execution::completion::settle_budget(reservation, reserved).await
                    {
                        tracing::error!(%provider, %model, ?error, "failed to retain unknown Responses budget");
                    }
                }
            };
            let usage_record = async {
                if let Some(key_id) = context.api_key_id()
                    && let Err(error) = keys
                        .record_usage_record(
                            key_id,
                            crate::core::keys::UsageRecord::unpriced(
                                usage.map_or(0, |usage| u64::from(usage.total_tokens)),
                                0.0,
                                "responses_usage_unknown",
                            ),
                        )
                        .await
                {
                    tracing::error!(%key_id, %error, "failed to record unknown Responses usage");
                }
            };
            tokio::join!(budget_settlement, usage_record);
            return;
        }
        let settlement = spend::usage_spend_settlement_with_request_pricing(
            (&limits, &keys, context.api_key_id()),
            (provider, model, usage),
            pricing,
            reservation,
            key_reservation,
        )
        .with_pricing_usage(pricing_usage)
        .with_ledger_facts(facts);
        spend::record_completion_spend_with_reservation_with_policy(
            &budgeted.pricing(),
            &state.config().gateway.pricing,
            settlement,
        )
        .await;
    };
    match captured_facts {
        Some(facts) => crate::core::request_ledger::scope_facts(facts, work).await,
        None => work.await,
    }
}

#[cfg(test)]
#[path = "responses_native_tests.rs"]
mod tests;
