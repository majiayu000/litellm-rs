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
    let callback = super::callbacks::CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        &context,
    );
    let (mut call, lease) = super::execution::execute_stream_with_selected_deployment_matching(
        state.unified_router(),
        &model,
        ProviderCapability::Responses,
        |deployment| {
            previous.as_ref().is_none_or(|(record, _)| {
                record.deployment_id.as_deref() == Some(deployment.id.as_str())
                    && deployment.provider.native_response_binding() == record.deployment_binding
            })
        },
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
                    let budget_request = budget_request(&body, &model);
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
                        .reserve_for_call(|_| {
                            spend::reserve_chat_completion_budget_with_request_pricing(
                                &pricing,
                                &state.config().gateway.pricing,
                                &limits,
                                &provider_name,
                                &model,
                                spend::ChatCompletionBudgetRequest::from(&budget_request)
                                    .with_retained_prompt_tokens(retained_prompt_tokens),
                            )
                        })?;
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
                    let response = if compact {
                        provider.compact_response(body).await?
                    } else {
                        provider.native_response(body).await?
                    };
                    Ok(NativeCall {
                        callback,
                        response,
                        provider: provider_name,
                        model,
                        deployment,
                        pricing,
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
    if let Some(error) = failure {
        callback.fail(error.to_string(), "provider_error");
        lease.finish_failure(&error);
        return Err(error.into());
    }
    let value = match value {
        Ok(value) => value,
        Err(_) => {
            let error = ProviderError::response_parsing("responses", "Invalid Responses JSON");
            callback.fail(error.to_string(), "provider_error");
            lease.finish_failure(&error);
            return Err(error.into());
        }
    };
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
        let error = ProviderError::response_parsing("responses", "Invalid compaction response");
        callback.fail(error.to_string(), "provider_error");
        lease.finish_failure(&error);
        return Err(error.into());
    }
    if value.get("error").is_some_and(|error| !error.is_null())
        || value.get("status").and_then(Value::as_str) == Some("failed")
    {
        callback.fail("Upstream response failed", "provider_error");
        lease.finish_failure(&ProviderError::api_error(
            "responses",
            502,
            "Upstream response failed",
        ));
    } else {
        lease.finish_success(
            usage
                .as_ref()
                .map_or(0, |usage| u64::from(usage.total_tokens)),
        );
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
    callback.complete_usage(usage.as_ref(), "success");
    Ok(HttpResponse::Ok().json(value))
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
    let reasoning = optional_tokens(usage.pointer("/output_tokens_details/reasoning_tokens"))?;
    if reasoning > output {
        return None;
    }
    let mut normalized = crate::core::providers::shared::strict_usage(
        &[input],
        &[output],
        Some((total, &[input, output])),
        Some((cached, input)),
    )?;
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
    reservation: Option<UnifiedBudgetReservation>,
    key_reservation: Option<BudgetReservation>,
    facts: Option<SharedRequestLedgerFacts>,
) {
    let budgeted = &state.budgeted;
    let limits = budgeted.budget_limits();
    let keys = budgeted.key_manager();
    let settlement = spend::usage_spend_settlement_with_request_pricing(
        (&limits, &keys, context.api_key_id()),
        (provider, model, usage),
        pricing,
        reservation,
        key_reservation,
    )
    .with_ledger_facts(facts);
    spend::record_completion_spend_with_reservation_with_policy(
        &budgeted.pricing(),
        &state.config().gateway.pricing,
        settlement,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_usage_checks_totals_and_details_without_double_counting_reasoning() {
        let valid = json!({"usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15,"input_tokens_details":{"cached_tokens":4},"output_tokens_details":{"reasoning_tokens":2}}});
        let usage = response_usage(&valid).unwrap();
        assert_eq!(usage.total_tokens, 15);
        assert_eq!(usage.completion_tokens, 3);
        assert_eq!(usage.prompt_tokens_details.unwrap().cached_tokens, Some(4));
        assert_eq!(
            usage.completion_tokens_details.unwrap().reasoning_tokens,
            Some(2)
        );
        for (path, value) in [
            ("/usage/total_tokens", json!(14)),
            ("/usage/input_tokens", json!(-1)),
            ("/usage/input_tokens_details/cached_tokens", json!(13)),
            ("/usage/output_tokens_details/reasoning_tokens", json!(4)),
        ] {
            let mut invalid = valid.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(response_usage(&invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn budget_projection_includes_image_overhead_and_native_fields() {
        let body = json!({"input":[{"role":"user","content":[{"type":"input_image","image_url":"https://example.com/image.png"}]}],"tools":[{"type":"web_search"}],"max_output_tokens":10});
        let projected = budget_request(&body, "gpt-4o-mini");
        let Some(MessageContent::Parts(parts)) = &projected.messages[0].content else {
            panic!("missing parts");
        };
        assert!(matches!(&parts[1], ContentPart::ImageUrl { .. }));
        assert_eq!(projected.max_tokens, Some(10));
    }
    #[test]
    fn budget_projection_does_not_count_image_data_as_text() {
        let image_data = format!("data:image/png;base64,{}", "A".repeat(100_000));
        let body = json!({"instructions":"follow the schema", "input":[{"role":"user","content":[{"type":"input_text","text":"describe this"},{"type":"input_image","image_url":image_data,"detail":"low"}]}],"tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]});
        let projected = budget_request(&body, "gpt-4o-mini");
        let Some(MessageContent::Parts(parts)) = &projected.messages[0].content else {
            panic!("missing parts")
        };
        let ContentPart::Text { text } = &parts[0] else {
            panic!("missing text")
        };
        assert!(text.len() < 1_000);
        for expected in ["follow the schema", "describe this", "lookup", "parameters"] {
            assert!(text.contains(expected));
        }
        let ContentPart::ImageUrl { image_url } = &parts[1] else {
            panic!("missing image")
        };
        assert_eq!(image_url.url, image_data);
        assert_eq!(image_url.detail.as_deref(), Some("low"));
        assert_eq!(
            body.pointer("/input/0/content/1/image_url").unwrap(),
            &json!(image_data)
        );
    }
}
