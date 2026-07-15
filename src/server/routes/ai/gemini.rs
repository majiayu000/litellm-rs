//! Gemini SDK-compatible native generation routes.

use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, http::StatusCode, web};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::error;

use crate::core::budget::{BudgetReservation, UnifiedBudgetReservation};
use crate::core::providers::{GeminiNativeRequest, ProviderError};
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

use super::budgeted::{StreamingDeploymentLease, run_stream, run_unary};
use super::openai_errors;

mod provider;
mod spend;
use provider::{
    GeminiRouteProvider, ensure_gemini_provider_candidate_configured,
    gemini_gateway_error_to_provider_error, gemini_http_error, gemini_router_models,
    missing_gemini_provider_error, send_gemini_request,
};
use spend::{
    GeminiSpendState, extract_gemini_sse_usage, record_gemini_spend, settle_gemini_stream_spend,
};

const GEMINI_V1: &str = "v1";
const GEMINI_V1BETA: &str = "v1beta";
const GEMINI_MAX_JSON_BYTES: usize = 16 * 1024 * 1024;

macro_rules! gemini_route_handler {
    ($name:ident, $version:expr, $method:literal, $stream:expr) => {
        pub async fn $name(
            state: web::Data<AppState>,
            req: HttpRequest,
            model: web::Path<String>,
            request: web::Json<Value>,
        ) -> ActixResult<HttpResponse> {
            proxy_gemini_route(
                state.get_ref(),
                &req,
                $version,
                model.into_inner(),
                $method,
                $stream,
                request.into_inner(),
            )
            .await
        }
    };
}

gemini_route_handler!(
    gemini_generate_content_v1beta,
    GEMINI_V1BETA,
    "generateContent",
    false
);
gemini_route_handler!(
    gemini_stream_generate_content_v1beta,
    GEMINI_V1BETA,
    "streamGenerateContent",
    true
);
gemini_route_handler!(
    gemini_generate_content_v1,
    GEMINI_V1,
    "generateContent",
    false
);
gemini_route_handler!(
    gemini_stream_generate_content_v1,
    GEMINI_V1,
    "streamGenerateContent",
    true
);

async fn proxy_gemini_route(
    state: &AppState,
    req: &HttpRequest,
    api_version: &str,
    requested_model: String,
    method: &'static str,
    stream: bool,
    request: Value,
) -> ActixResult<HttpResponse> {
    match proxy_gemini_route_inner(
        state,
        req,
        api_version,
        requested_model,
        method,
        stream,
        request,
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Gemini SDK route error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

async fn proxy_gemini_route_inner(
    state: &AppState,
    req: &HttpRequest,
    api_version: &str,
    requested_model: String,
    method: &'static str,
    stream: bool,
    request: Value,
) -> Result<HttpResponse, GatewayError> {
    let mut context = ensure_gemini_route_authorized(state, req)?;
    if let Err(error) = super::token_policy::attach_api_key_token_limit(req, &mut context) {
        return Ok(openai_errors::gateway_error_response(&error));
    }
    let request =
        apply_gemini_api_key_output_token_limit(context.api_key_max_tokens_per_request(), request)?;
    validate_api_version(api_version)?;
    validate_method(method)?;
    validate_model_segment(&requested_model)?;
    validate_gemini_request_size(&request)?;
    super::context::enforce_api_key_model_and_token_limits(
        req,
        &requested_model,
        gemini_requested_max_output_tokens(&request),
    )?;

    ensure_gemini_provider_candidate_configured(state.config().providers(), &requested_model)?;
    if stream {
        return proxy_gemini_stream_route_inner(
            state,
            context,
            api_version,
            requested_model,
            method,
            request,
        )
        .await;
    }

    let router_models = gemini_router_models(state.config().providers(), &requested_model);

    let mut last_router_error = None;
    for router_model in router_models {
        let result = run_unary(
            &state.unified_router,
            &router_model,
            gemini_route_capability(),
            {
                let context = context.clone();
                let request = request.clone();
                let requested_model = requested_model.clone();
                move |selected_provider, _selected_model, _selected_deployment_id| {
                    let context = context.clone();
                    let request = request.clone();
                    let requested_model = requested_model.clone();
                    async move {
                        let provider =
                            GeminiRouteProvider::selected(&selected_provider, &requested_model);
                        let (budget_reservation, key_budget_reservation, response) =
                            send_gemini_request(
                                state,
                                &selected_provider,
                                &provider,
                                GeminiNativeRequest {
                                    api_version: api_version.to_string(),
                                    model: requested_model,
                                    method,
                                    stream: false,
                                    body: request,
                                },
                                context.api_key_budget_id(),
                            )
                            .await?;

                        let response = gemini_upstream_response_to_http_response(
                            state,
                            GeminiUpstreamResponseParts {
                                context,
                                provider,
                                budget_reservation,
                                key_budget_reservation,
                                response,
                                stream,
                                stream_lease: None,
                            },
                        )
                        .await
                        .map_err(gemini_gateway_error_to_provider_error)?;
                        Ok((response, 0))
                    }
                }
            },
        )
        .await;

        match result {
            Ok(response) => return Ok(response),
            Err(GatewayError::Provider(ProviderError::QuotaExceeded {
                provider: "budget",
                message,
            })) if message.starts_with("provider ") => {
                last_router_error = Some(GatewayError::Provider(ProviderError::QuotaExceeded {
                    provider: "budget",
                    message,
                }));
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_router_error.unwrap_or_else(|| missing_gemini_provider_error(&requested_model)))
}

async fn proxy_gemini_stream_route_inner(
    state: &AppState,
    context: crate::core::types::context::RequestContext,
    api_version: &str,
    requested_model: String,
    method: &'static str,
    request: Value,
) -> Result<HttpResponse, GatewayError> {
    let router_models = gemini_router_models(state.config().providers(), &requested_model);
    let api_key_budget_id = context.api_key_budget_id();
    let mut last_router_error = None;
    for router_model in router_models {
        let result = run_stream(
            state.unified_router.clone(),
            &router_model,
            gemini_route_capability(),
            {
                let request = request.clone();
                let requested_model = requested_model.clone();
                move |selected_provider, _selected_model, _selected_deployment_id| {
                    let request = request.clone();
                    let requested_model = requested_model.clone();
                    async move {
                        let provider =
                            GeminiRouteProvider::selected(&selected_provider, &requested_model);
                        let (budget_reservation, key_budget_reservation, response) =
                            send_gemini_request(
                                state,
                                &selected_provider,
                                &provider,
                                GeminiNativeRequest {
                                    api_version: api_version.to_string(),
                                    model: requested_model,
                                    method,
                                    stream: true,
                                    body: request,
                                },
                                api_key_budget_id,
                            )
                            .await?;
                        Ok((
                            provider,
                            budget_reservation,
                            key_budget_reservation,
                            response,
                        ))
                    }
                }
            },
        )
        .await;

        match result {
            Ok(((provider, budget_reservation, key_budget_reservation, response), lease)) => {
                return gemini_upstream_response_to_http_response(
                    state,
                    GeminiUpstreamResponseParts {
                        context,
                        provider,
                        budget_reservation,
                        key_budget_reservation,
                        response,
                        stream: true,
                        stream_lease: Some(lease),
                    },
                )
                .await;
            }
            Err(GatewayError::Provider(ProviderError::QuotaExceeded {
                provider: "budget",
                message,
            })) if message.starts_with("provider ") => {
                last_router_error = Some(GatewayError::Provider(ProviderError::QuotaExceeded {
                    provider: "budget",
                    message,
                }));
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_router_error.unwrap_or_else(|| missing_gemini_provider_error(&requested_model)))
}

struct GeminiUpstreamResponseParts {
    context: crate::core::types::context::RequestContext,
    provider: GeminiRouteProvider,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    response: reqwest::Response,
    stream: bool,
    stream_lease: Option<StreamingDeploymentLease>,
}

async fn gemini_upstream_response_to_http_response(
    state: &AppState,
    parts: GeminiUpstreamResponseParts,
) -> Result<HttpResponse, GatewayError> {
    let GeminiUpstreamResponseParts {
        context,
        provider,
        budget_reservation,
        key_budget_reservation,
        response,
        stream,
        stream_lease,
    } = parts;

    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|error| GatewayError::internal(format!("Invalid upstream status: {error}")))?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or(if stream {
            "text/event-stream"
        } else {
            "application/json"
        })
        .to_string();

    if stream {
        let stream_lease = stream_lease
            .ok_or_else(|| GatewayError::internal("Gemini stream missing deployment lease"))?;
        return Ok(gemini_streaming_response(
            state,
            GeminiStreamResponseParts {
                context,
                provider,
                budget_reservation,
                key_budget_reservation,
                response,
                status,
                content_type,
                stream_lease,
            },
        ));
    }

    let body = response.bytes().await.map_err(gemini_http_error)?;
    if status.is_success() {
        let config = state.config();
        let budgeted = state.budgeted.clone();
        let pricing = budgeted.pricing();
        let budget_limits = budgeted.budget_limits();
        let key_manager = budgeted.key_manager();
        let spend_state = GeminiSpendState {
            pricing: pricing.as_ref(),
            pricing_config: &config.gateway.pricing,
            budget_limits: budget_limits.as_ref(),
            key_manager: &key_manager,
            api_key_id: context.api_key_id(),
        };
        record_gemini_spend(
            &spend_state,
            &provider,
            &body,
            budget_reservation,
            key_budget_reservation,
            true,
        )
        .await;
    }

    Ok(HttpResponse::build(status)
        .insert_header((actix_web::http::header::CONTENT_TYPE, content_type))
        .body(body))
}

struct GeminiStreamResponseParts {
    context: crate::core::types::context::RequestContext,
    provider: GeminiRouteProvider,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
    response: reqwest::Response,
    status: StatusCode,
    content_type: String,
    stream_lease: StreamingDeploymentLease,
}

fn gemini_streaming_response(state: &AppState, parts: GeminiStreamResponseParts) -> HttpResponse {
    let GeminiStreamResponseParts {
        context,
        provider,
        mut budget_reservation,
        mut key_budget_reservation,
        response,
        status,
        content_type,
        stream_lease,
    } = parts;
    let (tx, rx) = mpsc::channel::<Bytes>(8);
    let budgeted = state.budgeted.clone();
    let pricing = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let budget_limits = budgeted.budget_limits();
    let key_manager = budgeted.key_manager();
    let api_key_id = context.api_key_id();
    let should_record_spend = status.is_success();

    tokio::spawn(async move {
        let mut stream_lease = Some(stream_lease);
        let mut upstream = response.bytes_stream();
        let mut sse_buffer = String::new();
        let mut final_usage = None;
        let mut saw_upstream_output = false;

        while let Some(chunk_result) = upstream.next().await {
            let bytes = match chunk_result {
                Ok(bytes) => bytes,
                Err(_) => {
                    error!("Gemini SDK upstream stream error; closing client stream");
                    if let Some(lease) = stream_lease.take() {
                        let error = ProviderError::streaming_error(
                            "gemini_proxy",
                            "streamGenerateContent",
                            None,
                            None,
                            "Gemini upstream stream error",
                        );
                        lease.finish_failure(&error);
                    }
                    let spend_state = GeminiSpendState {
                        pricing: pricing.as_ref(),
                        pricing_config: &pricing_config,
                        budget_limits: &budget_limits,
                        key_manager: &key_manager,
                        api_key_id,
                    };
                    settle_gemini_stream_spend(
                        &spend_state,
                        &provider,
                        should_record_spend.then(|| final_usage.take()).flatten(),
                        budget_reservation.take(),
                        key_budget_reservation.take(),
                        false,
                    )
                    .await;
                    if tx
                        .send(Bytes::from_static(
                            b"event: error\ndata: Gemini upstream stream error\n\n",
                        ))
                        .await
                        .is_err()
                    {
                        error!("client disconnected before Gemini stream error could be sent");
                    }
                    return;
                }
            };
            saw_upstream_output = true;
            if let Some(usage) = extract_gemini_sse_usage(&bytes, &mut sse_buffer) {
                final_usage = Some(usage);
            }
            if tx.send(bytes).await.is_err() {
                let spend_state = GeminiSpendState {
                    pricing: pricing.as_ref(),
                    pricing_config: &pricing_config,
                    budget_limits: &budget_limits,
                    key_manager: &key_manager,
                    api_key_id,
                };
                settle_gemini_stream_spend(
                    &spend_state,
                    &provider,
                    should_record_spend.then(|| final_usage.take()).flatten(),
                    budget_reservation.take(),
                    key_budget_reservation.take(),
                    should_record_spend && saw_upstream_output,
                )
                .await;
                return;
            }
        }

        let spend_state = GeminiSpendState {
            pricing: pricing.as_ref(),
            pricing_config: &pricing_config,
            budget_limits: &budget_limits,
            key_manager: &key_manager,
            api_key_id,
        };
        let tokens_used = final_usage
            .as_ref()
            .map(|usage| u64::from(usage.total_tokens))
            .unwrap_or(0);
        settle_gemini_stream_spend(
            &spend_state,
            &provider,
            if should_record_spend {
                final_usage
            } else {
                None
            },
            budget_reservation.take(),
            key_budget_reservation.take(),
            should_record_spend && saw_upstream_output,
        )
        .await;
        if let Some(lease) = stream_lease.take() {
            lease.finish_success(tokens_used);
        }
    });

    let upstream =
        tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok::<_, actix_web::error::Error>);
    HttpResponse::build(status)
        .insert_header((actix_web::http::header::CONTENT_TYPE, content_type))
        .streaming(upstream)
}

fn gemini_route_capability() -> ProviderCapability {
    ProviderCapability::GeminiGenerateContent
}

fn ensure_gemini_route_authorized(
    state: &AppState,
    req: &HttpRequest,
) -> Result<crate::core::types::context::RequestContext, GatewayError> {
    let context = super::context::get_request_context(req)
        .map_err(|_| GatewayError::Auth("Unauthorized".to_string()))?;
    let auth = &state.config().gateway.auth;
    if !auth.enable_jwt && !auth.enable_api_key && auth.allow_anonymous {
        return Ok(context);
    }

    let user = super::context::get_authenticated_user(req);
    let api_key = super::context::get_authenticated_api_key(req);
    if super::context::check_permission(user.as_ref(), api_key.as_ref(), "chat") {
        Ok(context)
    } else {
        Err(GatewayError::Auth("Unauthorized".to_string()))
    }
}

fn validate_api_version(api_version: &str) -> Result<(), GatewayError> {
    if matches!(api_version, GEMINI_V1 | GEMINI_V1BETA) {
        return Ok(());
    }

    Err(GatewayError::validation("Unsupported Gemini API version"))
}

fn validate_method(method: &str) -> Result<(), GatewayError> {
    if matches!(method, "generateContent" | "streamGenerateContent") {
        return Ok(());
    }

    Err(GatewayError::validation("Unsupported Gemini method"))
}

fn validate_model_segment(model: &str) -> Result<(), GatewayError> {
    if model.trim().is_empty() {
        return Err(GatewayError::validation("Gemini model is required"));
    }

    if model
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Ok(());
    }

    Err(GatewayError::validation(
        "Gemini model must be a single safe path segment",
    ))
}

fn validate_gemini_request_size(request: &Value) -> Result<(), GatewayError> {
    let len = serde_json::to_vec(request)?.len();
    if len > GEMINI_MAX_JSON_BYTES {
        return Err(GatewayError::validation("Gemini request body too large"));
    }
    Ok(())
}

fn gemini_requested_max_output_tokens(request: &Value) -> Option<u32> {
    request
        .pointer("/generationConfig/maxOutputTokens")
        .and_then(Value::as_u64)
        .and_then(|tokens| u32::try_from(tokens).ok())
}

fn apply_gemini_api_key_output_token_limit(
    max_tokens_per_request: Option<u32>,
    mut request: Value,
) -> Result<Value, GatewayError> {
    let Some(limit) = max_tokens_per_request else {
        return Ok(request);
    };

    if let Some(requested) = gemini_requested_max_output_tokens(&request)
        && requested > limit
    {
        return Err(GatewayError::validation(format!(
            "requested token limit {requested} exceeds API key max_tokens_per_request {limit}"
        )));
    }

    let Some(object) = request.as_object_mut() else {
        return Err(GatewayError::validation(
            "Gemini request body must be a JSON object",
        ));
    };
    let generation_config = object
        .entry("generationConfig")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let Some(generation_config) = generation_config.as_object_mut() else {
        return Err(GatewayError::validation(
            "generationConfig must be a JSON object",
        ));
    };
    generation_config
        .entry("maxOutputTokens")
        .or_insert_with(|| Value::from(limit));

    Ok(request)
}
