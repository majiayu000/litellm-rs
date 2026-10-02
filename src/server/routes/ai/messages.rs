//! Native Anthropic Messages through the gateway's existing policy and spend chain.
use super::{budgeted, callbacks::CallbackLifecycle, execution::StreamingDeploymentLease, spend};
use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::models::openai::{
    ChatCompletionRequest, ChatMessage, MessageContent, MessageRole,
};
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::{Provider, ProviderError};
use crate::core::request_ledger::SharedRequestLedgerFacts;
use crate::core::types::{
    context::RequestContext,
    model::ProviderCapability,
    responses::{PromptTokensDetails, Usage},
};
use crate::server::{
    guardrails::{self, GuardrailDecisionSink},
    state::AppState,
};
use crate::utils::error::gateway_error::{
    GatewayError, current_error_response_request_id, gateway_http_error_facts,
};
use actix_web::{HttpRequest, HttpResponse, web};
use serde_json::{Value, json};
#[path = "messages_stream.rs"]
mod stream;
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

pub async fn create_message(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<Value>,
) -> HttpResponse {
    match create(state.get_ref(), &req, body.into_inner()).await {
        Ok(response) => response,
        Err(error) => error_response(&error),
    }
}

struct MessageCall {
    response: reqwest::Response,
    callback: CallbackLifecycle,
    provider: String,
    model: String,
    deployment: String,
    pricing: spend::RequestPricing,
    reservation: Option<UnifiedBudgetReservation>,
    key_reservation: Option<BudgetReservation>,
}

async fn create(
    state: &AppState,
    req: &HttpRequest,
    mut body: Value,
) -> Result<HttpResponse, GatewayError> {
    let mut context = super::context::get_request_context(req)
        .map_err(|_| GatewayError::Auth("Unauthorized".into()))?;
    let config = state.config();
    let auth = &config.gateway.auth;
    if (auth.enable_jwt || auth.enable_api_key || !auth.allow_anonymous)
        && !super::context::check_permission(
            super::context::get_authenticated_user(req).as_ref(),
            super::context::get_authenticated_api_key(req).as_ref(),
            "messages",
        )
    {
        return Err(GatewayError::Auth("Unauthorized".into()));
    }
    super::token_policy::attach_api_key_token_limit(req, &mut context)?;
    let requested_model = body
        .get("model")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| GatewayError::validation("model is required"))?
        .to_string();
    let max_tokens = body
        .get("max_tokens")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| GatewayError::validation("max_tokens must be a non-negative integer"))?;
    super::context::enforce_api_key_model_and_token_limits(
        req,
        &requested_model,
        Some(max_tokens),
    )?;
    if body
        .get("messages")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        return Err(GatewayError::validation(
            "messages must be a nonempty array",
        ));
    }
    let streaming = match body.get("stream") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err(GatewayError::validation("stream must be a boolean")),
    };
    if streaming {
        guardrails::reject_unsupported_streaming_mask(state)?;
    }
    let read_header = |name| {
        req.headers()
            .get(name)
            .map(|value| {
                value
                    .to_str()
                    .map(str::to_string)
                    .map_err(|_| GatewayError::validation("Invalid Anthropic protocol header"))
            })
            .transpose()
    };
    let version = read_header("anthropic-version")?;
    let beta = read_header("anthropic-beta")?;
    let sink = GuardrailDecisionSink::from_state(state, Some(&requested_model), None, None);
    body =
        guardrails::apply_native_messages(state.guardrails().as_ref(), body, false, &sink).await?;
    let model = state.unified_router().resolve_model_name(&requested_model);
    let callback = CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        &context,
    );
    let (call, lease) = super::execution::execute_stream_with_selected_deployment_matching(
        state.unified_router(),
        &model,
        ProviderCapability::ChatCompletion,
        |deployment| matches!(&deployment.provider, Provider::Anthropic(_)),
        {
            let context = context.clone();
            let callback = callback.clone();
            move |provider, model, deployment| {
                let mut body = body.clone();
                let context = context.clone();
                let callback = callback.clone();
                let version = version.clone();
                let beta = beta.clone();
                async move {
                    body["model"] = json!(model);
                    let provider_name = provider.name().to_string();
                    let pricing = spend::request_pricing_for_provider(
                        &state.budgeted.pricing(),
                        &provider,
                        &model,
                        ProviderCapability::ChatCompletion,
                    )?;
                    let budget_request = ChatCompletionRequest {
                        model: model.clone(),
                        messages: vec![ChatMessage {
                            role: MessageRole::User,
                            content: Some(MessageContent::Text(body.to_string())),
                            name: None,
                            function_call: None,
                            tool_calls: None,
                            tool_call_id: None,
                            audio: None,
                        }],
                        max_tokens: Some(max_tokens),
                        ..Default::default()
                    };
                    let Provider::Anthropic(native) = provider else {
                        return Err(ProviderError::not_supported(
                            "anthropic",
                            "Native Messages for this provider",
                        ));
                    };
                    let limits = state.budgeted.budget_limits();
                    let (response, reservations) = state
                        .budgeted
                        .for_selected_with_api_key_budget(
                            provider_name.clone(),
                            model.clone(),
                            context.api_key_budget_id(),
                            budgeted::ApiKeyBudgetPolicy::FromProviderReservation,
                        )
                        .reserve_call(
                            |_| {
                                spend::reserve_chat_completion_budget_with_request_pricing(
                                    &pricing,
                                    &state.config().gateway.pricing,
                                    &limits,
                                    &provider_name,
                                    &model,
                                    spend::ChatCompletionBudgetRequest::from(&budget_request),
                                )
                            },
                            || {
                                callback.begin_provider_execution_with_pricing(
                                    &provider_name,
                                    &model,
                                    pricing.clone(),
                                );
                                native.native_messages(body, version, beta)
                            },
                        )
                        .await?;
                    let (reservation, key_reservation) = reservations.into_parts();
                    Ok(MessageCall {
                        response,
                        callback,
                        provider: provider_name,
                        model,
                        deployment,
                        pricing,
                        reservation,
                        key_reservation,
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
    let MessageCall {
        mut response,
        callback,
        provider,
        model,
        deployment,
        pricing,
        reservation,
        key_reservation,
    } = call;
    let result = read_json(&mut response).await;
    let usage = result
        .as_ref()
        .ok()
        .and_then(|value| native_usage(value.get("usage")?));
    settle(
        state,
        &context,
        &provider,
        &model,
        pricing,
        usage.as_ref(),
        reservation,
        key_reservation,
        crate::core::request_ledger::current_facts(),
    )
    .await;
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            callback.fail(error.to_string(), "provider_error");
            if let GatewayError::Provider(provider_error) = &error {
                lease.finish_failure(provider_error);
            }
            return Err(error);
        }
    };
    if value
        .get("id")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
        || value.get("role").and_then(Value::as_str) != Some("assistant")
        || usage.is_none()
        || value.get("type").and_then(Value::as_str) != Some("message")
        || value.get("content").and_then(Value::as_array).is_none()
    {
        let error = ProviderError::response_parsing("anthropic", "Invalid Messages response");
        callback.fail(error.to_string(), "provider_error");
        lease.finish_failure(&error);
        return Err(error.into());
    }
    lease.finish_success(
        usage
            .as_ref()
            .map_or(0, |usage| u64::from(usage.normalized.total_tokens)),
    );
    let sink =
        GuardrailDecisionSink::from_state(state, Some(&model), Some(&provider), Some(&deployment));
    let value = guardrails::apply_native_messages(state.guardrails().as_ref(), value, true, &sink)
        .await
        .inspect_err(|error| {
            callback.fail(error.to_string(), "guardrail_error");
        })?;
    callback.complete_pricing_usage(usage.as_ref().map(|usage| &usage.pricing), "success");
    Ok(HttpResponse::Ok().json(value))
}

struct NativeUsage {
    normalized: Usage,
    pricing: PricingUsage,
}
fn native_usage(value: &Value) -> Option<NativeUsage> {
    for name in ["cache_creation", "server_tool_use"] {
        if value
            .get(name)
            .is_some_and(|value| !value.is_null() && !value.is_object())
        {
            return None;
        }
    }
    let count = |name: &str| {
        value.get(name).map_or(Some(0), |v| {
            if v.is_null() {
                Some(0)
            } else {
                v.as_u64().and_then(|v| u32::try_from(v).ok())
            }
        })
    };
    let input = value
        .get("input_tokens")?
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())?;
    let output = value
        .get("output_tokens")?
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())?;
    let creation = count("cache_creation_input_tokens")?;
    let read = count("cache_read_input_tokens")?;
    let prompt = input.checked_add(creation)?.checked_add(read)?;
    let total = prompt.checked_add(output)?;
    let nested = |path: &str| {
        value
            .pointer(path)
            .map_or(Some(0), |v| v.as_u64().and_then(|v| u32::try_from(v).ok()))
    };
    let one_hour = nested("/cache_creation/ephemeral_1h_input_tokens")?;
    let five_minute = nested("/cache_creation/ephemeral_5m_input_tokens")?;
    if value.get("cache_creation").is_some_and(|v| !v.is_null())
        && one_hour.checked_add(five_minute)? != creation
    {
        return None;
    }
    let normalized = Usage {
        prompt_tokens: prompt,
        completion_tokens: output,
        total_tokens: total,
        prompt_tokens_details: Some(PromptTokensDetails {
            cached_tokens: Some(read),
            cache_creation_tokens: Some(creation),
            cache_read_tokens: Some(read),
            audio_tokens: None,
        }),
        completion_tokens_details: None,
        thinking_usage: None,
    };
    let mut pricing = PricingUsage::from(&normalized);
    pricing.cache_creation_1h_tokens = Some(one_hour);
    pricing.web_search_requests = Some(nested("/server_tool_use/web_search_requests")?);
    Some(NativeUsage {
        normalized,
        pricing,
    })
}

#[allow(clippy::too_many_arguments)]
async fn settle(
    state: &AppState,
    context: &RequestContext,
    provider: &str,
    model: &str,
    pricing: spend::RequestPricing,
    usage: Option<&NativeUsage>,
    reservation: Option<UnifiedBudgetReservation>,
    key_reservation: Option<BudgetReservation>,
    facts: Option<SharedRequestLedgerFacts>,
) {
    let limits = state.budgeted.budget_limits();
    let keys = state.budgeted.key_manager();
    let settlement = spend::usage_spend_settlement_with_request_pricing(
        (&limits, &keys, context.api_key_id()),
        (provider, model, usage.map(|u| &u.normalized)),
        pricing,
        reservation,
        key_reservation,
    )
    .with_pricing_usage(usage.map(|u| u.pricing.clone()))
    .with_ledger_facts(facts);
    spend::record_completion_spend_with_reservation_with_policy(
        &state.budgeted.pricing(),
        &state.config().gateway.pricing,
        settlement,
    )
    .await;
}

async fn read_json(response: &mut reqwest::Response) -> Result<Value, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ProviderError::network("anthropic", "Messages body interrupted"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(ProviderError::response_parsing(
                "anthropic",
                "Messages body exceeds size limit",
            )
            .into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| ProviderError::response_parsing("anthropic", "Invalid Messages JSON").into())
}

pub(crate) fn error_response(error: &GatewayError) -> HttpResponse {
    let facts = gateway_http_error_facts(error);
    let message = match error {
        GatewayError::Provider(error) => error.redacted().to_string(),
        _ => error.to_string(),
    };
    let kind = match facts.status.as_u16() {
        400 | 402 | 409 | 422 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        413 => "request_too_large",
        429 => "rate_limit_error",
        529 => "overloaded_error",
        _ => "api_error",
    };
    let mut response = HttpResponse::build(facts.status);
    if let Some(seconds) = facts.headers.retry_after {
        response.insert_header(("retry-after", seconds.to_string()));
    }
    response.json(json!({"type":"error","error":{"type":kind,"message":message},"request_id":current_error_response_request_id()}))
}
