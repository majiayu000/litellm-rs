//! Native Anthropic Messages through the gateway's existing policy and spend chain.
use super::{budgeted, callbacks::CallbackLifecycle, execution::StreamingDeploymentLease, spend};
use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::{Provider, ProviderError};
use crate::core::request_ledger::SharedRequestLedgerFacts;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
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
    require_inference_geo: bool,
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
            "chat",
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
    validate_billing_scope(&body)?;
    let model = state.unified_router().resolve_model_name(&requested_model);
    let callback = CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        &context,
    );
    let (call, mut lease) = super::execution::execute_stream_with_selected_deployment_matching(
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
                    let Provider::Anthropic(native) = provider else {
                        return Err(ProviderError::not_supported(
                            "anthropic",
                            "Native Messages for this provider",
                        ));
                    };
                    use crate::core::providers::anthropic::models::{
                        AnthropicModelFamily, get_anthropic_registry,
                    };
                    let require_inference_geo =
                        match get_anthropic_registry().get_model_family(&model) {
                            Some(
                                AnthropicModelFamily::ClaudeOpus45
                                | AnthropicModelFamily::ClaudeSonnet45
                                | AnthropicModelFamily::ClaudeHaiku45,
                            ) => false,
                            Some(_) => true,
                            None => {
                                return Err(ProviderError::not_supported(
                                    "anthropic",
                                    "Unverified native Messages model billing",
                                ));
                            }
                        };
                    let count_body = token_count_body(&body);
                    let mut counted = native
                        .native_count_tokens(count_body, version.clone(), beta.clone())
                        .await?;
                    let counted = read_json(&mut counted).await.map_err(|error| match error {
                        GatewayError::Provider(error) => error,
                        _ => ProviderError::response_parsing(
                            "anthropic",
                            "Invalid token count response",
                        ),
                    })?;
                    let input_tokens = counted
                        .get("input_tokens")
                        .and_then(Value::as_u64)
                        .and_then(|tokens| u32::try_from(tokens).ok())
                        .ok_or_else(|| {
                            ProviderError::response_parsing(
                                "anthropic",
                                "Invalid input_tokens count",
                            )
                        })?;
                    let context_limit = native
                        .models()
                        .iter()
                        .find(|info| info.id == model)
                        .map(|info| info.max_context_length);
                    let mut estimated_usage =
                        reservation_usage(&body, input_tokens, max_tokens, context_limit)?;
                    // Omission inherits a private workspace default. Reserve the
                    // highest supported geo without changing the native request.
                    if require_inference_geo {
                        estimated_usage.inference_geo = Some(
                            body.get("inference_geo")
                                .and_then(Value::as_str)
                                .unwrap_or("us")
                                .to_string(),
                        );
                    }
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
                            async |_| {
                                spend::reserve_pricing_usage_budget_with_request_pricing(
                                    &pricing,
                                    &state.config().gateway.pricing,
                                    &limits,
                                    &provider_name,
                                    &model,
                                    &estimated_usage,
                                )
                                .await
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
                        require_inference_geo,
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
        require_inference_geo,
    } = call;
    let result = read_json(&mut response).await;
    let usage = result
        .as_ref()
        .ok()
        .and_then(|value| native_usage(value.get("usage")?, require_inference_geo));
    let result = result.and_then(|value| {
        if !valid_message_envelope(&value)
            || usage.is_none()
            || value
                .get("stop_reason")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            Err(ProviderError::response_parsing("anthropic", "Invalid Messages response").into())
        } else {
            Ok(value)
        }
    });
    let tokens_used = usage
        .as_ref()
        .map_or(0, |usage| u64::from(usage.normalized.total_tokens));
    let settlement = settle(
        state,
        &context,
        &provider,
        &model,
        pricing,
        usage.as_ref(),
        reservation,
        key_reservation,
        crate::core::request_ledger::current_facts(),
    );
    match result.as_ref() {
        Ok(_) => lease.settle_terminal(tokens_used, None, settlement).await,
        Err(GatewayError::Provider(error)) => {
            lease
                .settle_terminal(tokens_used, Some(error), settlement)
                .await
        }
        Err(_) => {
            lease
                .settle_interrupted(tokens_used, None, settlement)
                .await
        }
    }
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            callback.fail(error.to_string(), "provider_error");
            if let GatewayError::Provider(provider_error) = &error {
                lease
                    .finish_failure_with_tokens(provider_error, tokens_used)
                    .await;
            }
            return Err(error);
        }
    };
    lease
        .finish_success(
            usage
                .as_ref()
                .map_or(0, |usage| u64::from(usage.normalized.total_tokens)),
        )
        .await;
    let sink =
        GuardrailDecisionSink::from_state(state, Some(&model), Some(&provider), Some(&deployment));
    let value = guardrails::apply_native_messages(state.guardrails().as_ref(), value, true, &sink)
        .await
        .inspect_err(|error| {
            callback.fail(error.to_string(), "guardrail_error");
        })?;
    callback.complete_pricing_usage(
        usage.as_ref().map(|usage| &usage.normalized),
        usage.as_ref().map(|usage| &usage.pricing),
        "success",
    );
    Ok(HttpResponse::Ok().json(value))
}

fn valid_message_envelope(value: &Value) -> bool {
    value
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty())
        && value.get("role").and_then(Value::as_str) == Some("assistant")
        && value.get("type").and_then(Value::as_str) == Some("message")
        && value.get("content").and_then(Value::as_array).is_some()
}

// Count the original structured input, including image/document sources and tool
// schemas, on the selected account. Generation-only fields are not count API fields.
fn token_count_body(body: &Value) -> Value {
    let mut count = serde_json::Map::new();
    for field in [
        "model",
        "messages",
        "system",
        "tools",
        "tool_choice",
        "thinking",
    ] {
        if let Some(value) = body.get(field) {
            count.insert(field.into(), value.clone());
        }
    }
    Value::Object(count)
}

// Native fields remain unchanged, but unsupported billing modes must fail before
// contacting either the token-count or generation endpoint.
fn validate_billing_scope(body: &Value) -> Result<(), GatewayError> {
    if body
        .get("speed")
        .is_some_and(|value| !value.is_null() && value.as_str() != Some("standard"))
    {
        return Err(GatewayError::validation(
            "Native Messages speed supports only standard until premium pricing is implemented",
        ));
    }
    if ["fallbacks", "compaction", "container"]
        .iter()
        .any(|field| body.get(*field).is_some_and(|value| !value.is_null()))
        || body
            .pointer("/context_management/edits")
            .and_then(Value::as_array)
            .is_some_and(|edits| {
                edits.iter().any(|edit| {
                    edit.get("type")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| kind.starts_with("compact_"))
                })
            })
    {
        return Err(GatewayError::validation(
            "Native Messages server-side fallback, compaction and containers require separate billing",
        ));
    }
    if body
        .get("inference_geo")
        .is_some_and(|value| !value.is_null() && !matches!(value.as_str(), Some("global" | "us")))
    {
        return Err(GatewayError::validation("Unsupported inference_geo"));
    }
    if body.get("mcp_servers").is_some_and(|value| {
        !value.is_null() && value.as_array().is_none_or(|servers| !servers.is_empty())
    }) {
        return Err(GatewayError::validation(
            "Native Messages hosted MCP billing is not supported",
        ));
    }
    for tool in body
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let kind = tool.get("type").and_then(Value::as_str).unwrap_or_default();
        if kind.starts_with("advisor_")
            || kind.starts_with("code_execution_")
            || kind.starts_with("tool_search_")
            || kind.starts_with("mcp_")
        {
            return Err(GatewayError::validation(
                "Native Messages runtime/hosted tool billing is not supported",
            ));
        }
        let callers = tool.get("allowed_callers");
        let direct = callers.is_some_and(|value| value == &json!(["direct"]));
        let web = kind.starts_with("web_search_") || kind.starts_with("web_fetch_");
        let basic_web = matches!(kind, "web_search_20250305" | "web_fetch_20250910");
        if (callers.is_some() && !direct) || (web && !basic_web && !direct) {
            return Err(GatewayError::validation(
                "Native Messages tools require allowed_callers [direct]; dynamic filtering and programmatic execution billing are not supported",
            ));
        }
    }
    Ok(())
}

fn reservation_usage(
    body: &Value,
    mut input: u32,
    output: u32,
    context_limit: Option<u32>,
) -> Result<PricingUsage, ProviderError> {
    let mut searches = 0_u32;
    let mut tool_turns = 0_u32;
    for tool in body
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let kind = tool.get("type").and_then(Value::as_str).unwrap_or_default();
        if kind.starts_with("web_search_") || kind.starts_with("web_fetch_") {
            let uses = tool
                .get("max_uses")
                .and_then(Value::as_u64)
                .and_then(|uses| u32::try_from(uses).ok())
                .ok_or_else(|| {
                    ProviderError::invalid_request(
                        "anthropic",
                        "Native web tools require an explicit max_uses budget bound",
                    )
                })?;
            tool_turns = tool_turns.checked_add(uses).ok_or_else(|| {
                ProviderError::invalid_request(
                    "anthropic",
                    "Web tool budget bound exceeds supported range",
                )
            })?;
            if kind.starts_with("web_search_") {
                searches = searches.checked_add(uses).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "anthropic",
                        "Web search budget bound exceeds supported range",
                    )
                })?;
            }
        }
    }
    if tool_turns > 0 {
        let context = context_limit.filter(|limit| *limit > 0).ok_or_else(|| {
            ProviderError::not_supported(
                "anthropic",
                "Web tool budget requires a verified model context limit",
            )
        })?;
        input = context
            .checked_mul(tool_turns)
            .and_then(|tokens| input.checked_add(tokens))
            .ok_or_else(|| {
                ProviderError::invalid_request(
                    "anthropic",
                    "Web tool token reservation exceeds supported range",
                )
            })?;
    }
    fn cache_ttl(value: &Value) -> u8 {
        match value {
            Value::Array(values) => values.iter().map(cache_ttl).max().unwrap_or(0),
            Value::Object(fields) => {
                let own = fields
                    .get("cache_control")
                    .filter(|v| v.is_object())
                    .map_or(0, |v| {
                        if v.get("ttl").and_then(Value::as_str) == Some("1h") {
                            2
                        } else {
                            1
                        }
                    });
                own.max(fields.get("content").map_or(0, cache_ttl))
            }
            _ => 0,
        }
    }
    let ttl = [
        Some(body),
        body.get("messages"),
        body.get("system"),
        body.get("tools"),
    ]
    .into_iter()
    .flatten()
    .map(cache_ttl)
    .max()
    .unwrap_or(0);
    Ok(PricingUsage {
        prompt_tokens: input,
        completion_tokens: output,
        web_search_requests: Some(searches),
        total_tokens: input.checked_add(output).ok_or_else(|| {
            ProviderError::invalid_request("anthropic", "Token count exceeds supported range")
        })?,
        // Reserving all input at the most expensive requested cache-write TTL
        // covers a cold cache. Settlement uses the actual category counts.
        cache_creation_tokens: (ttl > 0).then_some(input),
        cache_creation_1h_tokens: (ttl == 2).then_some(input),
        ..Default::default()
    })
}

struct NativeUsage {
    normalized: Usage,
    pricing: PricingUsage,
}
fn native_usage(value: &Value, require_inference_geo: bool) -> Option<NativeUsage> {
    let inference_geo = match value.get("inference_geo") {
        Some(Value::String(geo)) if matches!(geo.as_str(), "global" | "us") => Some(geo.clone()),
        None | Some(Value::Null) if !require_inference_geo => None,
        _ => return None,
    };
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
    pricing.inference_geo = inference_geo;
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
    if usage.is_none() {
        spend::capture_ledger_settlement(facts.as_ref(), provider, model, None, None);
        // Preserve the budget upper bound without presenting it as an actual bill.
        if let Some(reservation) = key_reservation {
            let reserved = reservation.reserved_amount();
            spend::settle_api_key_budget_reservation(
                Some(reservation),
                reserved,
                "Messages usage unknown",
            );
        }
        super::execution::completion::observe_usage(0);
        let budget_settlement = async {
            if let Some(reservation) = reservation {
                let reserved = reservation.reserved_amount();
                if let Err(error) =
                    super::execution::completion::settle_budget(reservation, reserved).await
                {
                    tracing::error!(%provider, %model, ?error, "failed to retain unknown Messages budget");
                }
            }
        };
        let usage_record = async {
            if let Some(key_id) = context.api_key_id()
                && let Err(error) = keys
                    .record_usage_record(
                        key_id,
                        crate::core::keys::UsageRecord::unpriced(0, 0.0, "messages_usage_unknown"),
                    )
                    .await
            {
                tracing::error!(%key_id, %error, "failed to record unknown Messages usage");
            }
        };
        tokio::join!(budget_settlement, usage_record);
        return;
    }
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
    let native = match error {
        GatewayError::Provider(provider_error) => match provider_error.redacted() {
            ProviderError::ApiError {
                provider: "anthropic",
                message,
                ..
            }
            | ProviderError::RateLimit {
                provider: "anthropic",
                message,
                ..
            } => serde_json::from_str::<Value>(&message)
                .ok()
                .and_then(|body| {
                    Some(json!({"type": body.pointer("/error/type")?.as_str()?,
                        "message": body.pointer("/error/message")?.as_str()?}))
                }),
            _ => None,
        },
        _ => None,
    };
    response.json(json!({"type":"error","error":native.unwrap_or_else(|| json!({"type":kind,"message":message})),"request_id":current_error_response_request_id()}))
}
