//! Image API endpoints

use crate::config::models::provider::ProviderConfig;
mod generation;
mod multipart;
mod pricing_keys;
mod proxy_spend;

use crate::core::models::openai::ImageGenerationRequest;
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::base::ProviderRequestBuilder;
use crate::core::providers::{Provider, ProviderError};
use crate::core::types::context::RequestContext;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{
    HttpRequest, HttpResponse, Result as ActixResult, http::StatusCode, http::header, web,
};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::Url;
use reqwest::header::{HeaderName, HeaderValue};
use tracing::{error, info};

use super::budgeted::{ApiKeyBudgetPolicy, run_unary};
use super::context::handle_ai_request;
use super::route_http::RouteHttpClient;
use super::{openai_errors, provider_config};
use multipart::{extract_text_field as extract_multipart_text_field, replace_text_field};
use proxy_spend::record_image_proxy_spend;

const OPENAI_IMAGE_BASE_URL: &str = "https://api.openai.com/v1";
const MAX_IMAGE_MULTIPART_BYTES: usize = 64 * 1024 * 1024;
#[derive(Debug, Clone, Copy)]
enum ImageProxyEndpoint {
    Edits,
    Variations,
}

#[derive(Debug, Clone)]
struct ImageProxyProvider {
    provider_name: String,
    base_url: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    client: RouteHttpClient,
}

#[derive(Debug, Clone)]
struct ImageProxyFormFields {
    model: Option<String>,
    prompt: Option<String>,
    size: Option<String>,
    quality: Option<String>,
    n: u32,
}

pub async fn image_generations(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ImageGenerationRequest>,
) -> ActixResult<HttpResponse> {
    let mut request = request.into_inner();
    info!("Image generation request for model: {:?}", request.model);

    let requested_model = match required_image_generation_model(&request) {
        Ok(model) => model.to_string(),
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };
    if let Err(error) =
        super::context::enforce_api_key_model_and_token_limits(&req, &requested_model, None)
    {
        return Ok(openai_errors::gateway_error_response(&error));
    }
    request.model = Some(state.unified_router.resolve_model_name(&requested_model));

    handle_ai_request(&req, request, "Image generation", |request, context| {
        generation::handle_image_generation_with_state(state.get_ref(), request, context)
    })
    .await
}

fn required_image_generation_model(request: &ImageGenerationRequest) -> Result<&str, GatewayError> {
    request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or_else(|| GatewayError::validation("model is required"))
}

pub async fn image_edits(
    state: web::Data<AppState>,
    req: HttpRequest,
    payload: web::Payload,
) -> ActixResult<HttpResponse> {
    match proxy_image_multipart_endpoint(state.get_ref(), &req, payload, ImageProxyEndpoint::Edits)
        .await
    {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Image edit error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

pub async fn image_variations(
    state: web::Data<AppState>,
    req: HttpRequest,
    payload: web::Payload,
) -> ActixResult<HttpResponse> {
    match proxy_image_multipart_endpoint(
        state.get_ref(),
        &req,
        payload,
        ImageProxyEndpoint::Variations,
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(error) => {
            error!("Image variation error: {}", error);
            Ok(openai_errors::gateway_error_response(&error))
        }
    }
}

async fn proxy_image_multipart_endpoint(
    state: &AppState,
    req: &HttpRequest,
    payload: web::Payload,
    endpoint: ImageProxyEndpoint,
) -> Result<HttpResponse, GatewayError> {
    let context = ensure_image_route_authorized(state, req)?;
    let content_type = image_multipart_content_type(req)?;
    let body = read_image_multipart_payload(payload).await?;
    let form_fields = extract_image_proxy_form_fields(&body, &content_type);
    let public_model = required_image_proxy_model(&form_fields)?;
    super::context::enforce_api_key_model_and_token_limits(req, public_model, None)?;
    let requested_model = state.unified_router.resolve_model_name(public_model);
    let body = if requested_model == public_model {
        body
    } else {
        replace_text_field(&body, &content_type, "model", &requested_model)?
    };
    let requested_model = requested_model.as_str();
    ensure_image_proxy_candidate_configured(
        state.config().gateway.providers.as_slice(),
        requested_model,
    )?;
    let budgeted = state.budgeted.clone();
    let pricing_service = budgeted.pricing();
    let budget_limits = budgeted.budget_limits();
    let key_manager = budgeted.key_manager();
    let router_models =
        image_proxy_router_models(state.config().gateway.providers.as_slice(), requested_model);
    let pricing_config = state.config().gateway.pricing.clone();
    let api_key_id = super::context::get_authenticated_api_key(req).map(|key| key.metadata.id);
    let api_key_budget_id = context.api_key_budget_id();

    let mut last_router_error = None;
    for router_model in router_models {
        let result = run_unary(
            &state.unified_router,
            &router_model,
            endpoint.capability(),
            {
                let budgeted = budgeted.clone();
                let budget_limits = budget_limits.clone();
                let key_manager = key_manager.clone();
                let pricing_config = pricing_config.clone();
                let body = body.clone();
                let content_type = content_type.clone();
                let pricing_service = pricing_service.clone();
                let form_fields = form_fields.clone();
                move |selected_provider, selected_model, _deployment_id| {
                    let budgeted = budgeted.clone();
                    let budget_limits = budget_limits.clone();
                    let key_manager = key_manager.clone();
                    let pricing_config = pricing_config.clone();
                    let body = body.clone();
                    let content_type = content_type.clone();
                    let pricing_service = pricing_service.clone();
                    let form_fields = form_fields.clone();
                    async move {
                        let mut request_pricing = super::spend::request_pricing_for_provider(
                            &pricing_service,
                            &selected_provider,
                            &selected_model,
                        )?;
                        let base_model = request_pricing
                            .estimation_model(&selected_model)
                            .to_string();
                        if let Some(variant) = pricing_keys::resolve_image_request_pricing(
                            &request_pricing,
                            &base_model,
                            form_fields.size.as_deref(),
                            form_fields.quality.as_deref(),
                        ) {
                            request_pricing = variant;
                        }
                        let usage = estimated_image_proxy_usage(&form_fields, &request_pricing);
                        let (estimated_cost, unpriced) = request_image_proxy_cost(
                            &request_pricing,
                            &pricing_config,
                            selected_provider.name(),
                            &selected_model,
                            &usage,
                        )?;
                        if estimated_cost <= 0.0 && !unpriced {
                            return Err(ProviderError::configuration(
                                "image_proxy",
                                format!("Image model '{selected_model}' has non-positive pricing"),
                            ));
                        }
                        let provider = selected_image_proxy_provider(
                            state.config().gateway.providers.as_slice(),
                            &selected_provider,
                            &selected_model,
                            requested_model,
                        )?;
                        let provider_for_call = provider.clone();
                        let (response, reservations) = budgeted
                            .for_selected_with_api_key_budget(
                                provider.provider_name.clone(),
                                requested_model.to_string(),
                                api_key_budget_id,
                                ApiKeyBudgetPolicy::RequirePricedReservation,
                            )
                            .with_precomputed_api_key_budget_cost(Some(estimated_cost))
                            .reserve_call(
                                |context| context.reserve_spend(estimated_cost).map(Some),
                                || async move {
                                    let url = image_proxy_url(&provider_for_call, endpoint)
                                        .map_err(image_proxy_gateway_error_to_provider_error)?;
                                    let request = provider_for_call
                                        .client
                                        .ordinary_post(url)
                                        .map_err(image_proxy_gateway_error_to_provider_error)?;
                                    let response =
                                        apply_image_proxy_headers(request, &provider_for_call)
                                            .header(reqwest::header::CONTENT_TYPE, content_type)
                                            .body(body)
                                            .send()
                                            .await
                                            .map_err(|error| {
                                                ProviderError::network(
                                                    "image_proxy",
                                                    error.to_string(),
                                                )
                                            })?;

                                    if !response.status().is_success() {
                                        return Err(image_proxy_upstream_error(response).await);
                                    }

                                    Ok(response)
                                },
                            )
                            .await?;
                        let (budget_reservation, key_budget_reservation) =
                            reservations.into_parts();

                        record_image_proxy_spend(
                            &pricing_config,
                            budget_limits.as_ref(),
                            &key_manager,
                            &provider,
                            requested_model,
                            &usage,
                            estimated_cost,
                            unpriced,
                            budget_reservation,
                            api_key_id,
                            key_budget_reservation,
                        )
                        .await;
                        let tokens_used = image_proxy_tokens_used(&usage);
                        let response = image_proxy_response_to_http_response(response)
                            .await
                            .map_err(image_proxy_gateway_error_to_provider_error)?;
                        Ok((response, tokens_used))
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

    Err(last_router_error.unwrap_or_else(missing_image_proxy_provider_error))
}

fn required_image_proxy_model(form_fields: &ImageProxyFormFields) -> Result<&str, GatewayError> {
    let model = form_fields
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .ok_or_else(|| GatewayError::validation("model is required"))?;
    Ok(model)
}

fn estimated_image_proxy_usage(
    form_fields: &ImageProxyFormFields,
    request_pricing: &super::spend::RequestPricing,
) -> PricingUsage {
    let prompt_tokens = form_fields
        .prompt
        .as_deref()
        .map(estimated_text_tokens)
        .unwrap_or(1);
    let image_tokens = estimated_image_output_tokens(
        form_fields.size.as_deref(),
        form_fields.quality.as_deref(),
        form_fields.n,
    );
    let mut usage = PricingUsage::new(prompt_tokens, 0);
    usage.image_tokens = Some(image_tokens);
    usage.output_image_count = Some(form_fields.n.max(1));
    usage.output_image_pricing_keys = request_pricing
        .priced_parts()
        .map(|(provider, model)| {
            pricing_keys::image_pricing_keys(
                provider,
                model,
                form_fields.size.as_deref(),
                form_fields.quality.as_deref(),
            )
        })
        .unwrap_or_default();
    usage
}

fn request_image_proxy_cost(
    request_pricing: &super::spend::RequestPricing,
    pricing_config: &crate::config::models::gateway::GatewayPricingConfig,
    provider: &str,
    model: &str,
    usage: &PricingUsage,
) -> Result<(f64, bool), ProviderError> {
    match request_pricing.calculate_usage(usage) {
        Ok(cost) => Ok((cost.total_cost, false)),
        Err(error) => match pricing_config.unpriced_model_policy {
            crate::config::models::gateway::UnpricedModelPolicy::Reject => {
                Err(super::spend::model_not_priced_error(provider, model, error))
            }
            crate::config::models::gateway::UnpricedModelPolicy::AllowUnpriced => Ok((
                super::spend::fallback_cost_for_usage(pricing_config, usage),
                true,
            )),
        },
    }
}

fn estimated_text_tokens(text: &str) -> u32 {
    u32::try_from(text.chars().count().div_ceil(4))
        .unwrap_or(u32::MAX)
        .max(1)
}

fn estimated_image_output_tokens(size: Option<&str>, quality: Option<&str>, quantity: u32) -> u32 {
    let base_tokens: u32 = match size.unwrap_or("1024x1024") {
        "256x256" => 256,
        "512x512" => 512,
        "1024x1792" | "1792x1024" => 1_792,
        "1024x1024" => 1_024,
        _ => 1_024,
    };
    let quality_multiplier: u32 = match quality.map(str::to_ascii_lowercase).as_deref() {
        Some("hd") | Some("high") => 2,
        _ => 1,
    };
    base_tokens
        .saturating_mul(quality_multiplier)
        .saturating_mul(quantity.max(1))
}

fn extract_image_proxy_form_fields(body: &Bytes, content_type: &str) -> ImageProxyFormFields {
    let n = extract_multipart_text_field(body, content_type, "n")
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1);

    ImageProxyFormFields {
        model: extract_multipart_text_field(body, content_type, "model"),
        prompt: extract_multipart_text_field(body, content_type, "prompt"),
        size: extract_multipart_text_field(body, content_type, "size"),
        quality: extract_multipart_text_field(body, content_type, "quality"),
        n,
    }
}

fn ensure_image_route_authorized(
    state: &AppState,
    req: &HttpRequest,
) -> Result<RequestContext, GatewayError> {
    let context = super::context::get_request_context(req)
        .map_err(|_| GatewayError::Auth("Unauthorized".to_string()))?;
    let auth = &state.config().gateway.auth;
    if !auth.enable_jwt && !auth.enable_api_key && auth.allow_anonymous {
        return Ok(context);
    }

    let user = super::context::get_authenticated_user(req);
    let api_key = super::context::get_authenticated_api_key(req);
    if super::context::check_permission(user.as_ref(), api_key.as_ref(), "images") {
        Ok(context)
    } else {
        Err(GatewayError::Auth("Unauthorized".to_string()))
    }
}

async fn read_image_multipart_payload(mut payload: web::Payload) -> Result<Bytes, GatewayError> {
    let mut body = Vec::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|error| {
            GatewayError::validation(format!("Invalid multipart data: {error}"))
        })?;
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| GatewayError::validation("Image multipart payload too large"))?;
        if next_len > MAX_IMAGE_MULTIPART_BYTES {
            return Err(GatewayError::validation(
                "Image multipart payload too large",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Bytes::from(body))
}

async fn image_proxy_response_to_http_response(
    response: reqwest::Response,
) -> Result<HttpResponse, GatewayError> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|error| GatewayError::internal(format!("Invalid upstream status: {error}")))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let body = response.bytes().await?;

    Ok(HttpResponse::build(status)
        .insert_header((header::CONTENT_TYPE, content_type))
        .body(body))
}

fn ensure_image_proxy_candidate_configured(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Result<(), GatewayError> {
    let candidates = image_proxy_candidate_configs(providers);
    if candidates.is_empty() {
        return Err(missing_image_proxy_provider_error());
    }

    if candidates
        .iter()
        .any(|provider| image_provider_supports_requested_model(provider, Some(requested_model)))
    {
        Ok(())
    } else {
        Err(GatewayError::Provider(ProviderError::model_not_found(
            "image_proxy",
            requested_model,
        )))
    }
}

fn image_proxy_router_models(providers: &[ProviderConfig], requested_model: &str) -> Vec<String> {
    let candidates = image_proxy_candidate_configs(providers);
    let mut router_models = Vec::new();
    if candidates.iter().any(|provider| {
        !provider.models.is_empty() && provider.models.iter().any(|model| model == requested_model)
    }) {
        router_models.push(requested_model.to_string());
    }

    let wildcard_models = candidates
        .into_iter()
        .filter(|provider| provider.models.is_empty())
        .map(|provider| provider.name.clone())
        .collect::<Vec<_>>();
    if wildcard_models.is_empty() {
        if router_models.is_empty() {
            router_models.push(requested_model.to_string());
        }
        router_models
    } else {
        router_models.extend(wildcard_models);
        router_models
    }
}

fn selected_image_proxy_provider(
    providers: &[ProviderConfig],
    selected_provider: &Provider,
    selected_model: &str,
    requested_model: &str,
) -> Result<ImageProxyProvider, ProviderError> {
    let provider_name = selected_provider.name();
    let candidates = image_proxy_candidate_configs(providers);
    let matching = candidates
        .iter()
        .copied()
        .filter(|provider| image_provider_supports_requested_model(provider, Some(requested_model)))
        .find(|provider| {
            provider.name == provider_name
                || provider
                    .settings
                    .get("provider_name")
                    .and_then(|value| value.as_str())
                    == Some(provider_name)
                || (provider.models.is_empty() && provider.name == selected_model)
        })
        .ok_or_else(|| {
            ProviderError::configuration(
                "image_proxy",
                format!(
                    "selected image provider '{provider_name}' for model '{selected_model}' has no matching gateway provider config"
                ),
            )
        })?;

    image_proxy_provider_from_config(matching).map_err(image_proxy_gateway_error_to_provider_error)
}

fn image_proxy_candidate_configs(providers: &[ProviderConfig]) -> Vec<&ProviderConfig> {
    providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_openai_image_provider(provider))
        .collect()
}

fn image_proxy_provider_from_config(
    provider: &ProviderConfig,
) -> Result<ImageProxyProvider, GatewayError> {
    if provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Image provider '{}' is missing api_key",
            provider.name
        )));
    }

    let base_url = image_proxy_base_url(provider)?;
    let client = RouteHttpClient::new(
        "image_proxy",
        base_url.clone(),
        provider.endpoint_access,
        provider.timeout,
    )?;
    Ok(ImageProxyProvider {
        provider_name: provider.name.clone(),
        base_url,
        headers: image_proxy_provider_headers(provider)?,
        client,
    })
}

fn image_proxy_gateway_error_to_provider_error(error: GatewayError) -> ProviderError {
    match error {
        GatewayError::Provider(error) => error,
        GatewayError::Validation(message) | GatewayError::BadRequest(message) => {
            ProviderError::invalid_request("image_proxy", message)
        }
        GatewayError::Config(message) => ProviderError::configuration("image_proxy", message),
        GatewayError::Auth(message) => ProviderError::authentication("image_proxy", message),
        GatewayError::Forbidden(message) => ProviderError::api_error("image_proxy", 403, message),
        GatewayError::Timeout(message) => ProviderError::timeout("image_proxy", message),
        GatewayError::RateLimit {
            message,
            retry_after,
            ..
        } => ProviderError::rate_limit_with_retry("image_proxy", message, retry_after),
        GatewayError::HttpClient(error) => ProviderError::network("image_proxy", error.to_string()),
        GatewayError::Network(message) => ProviderError::network("image_proxy", message),
        GatewayError::Unavailable(message) => {
            ProviderError::provider_unavailable("image_proxy", message)
        }
        other => ProviderError::api_error("image_proxy", 500, other.to_string()),
    }
}

async fn image_proxy_upstream_error(response: reqwest::Response) -> ProviderError {
    let status = response.status().as_u16();
    let message = response
        .text()
        .await
        .unwrap_or_else(|error| format!("failed to read image upstream error body: {error}"));

    match status {
        400 => ProviderError::invalid_request("image_proxy", message),
        401 => ProviderError::authentication("image_proxy", message),
        403 => ProviderError::api_error("image_proxy", status, message),
        402 => ProviderError::quota_exceeded("image_proxy", message),
        404 => ProviderError::model_not_found("image_proxy", message),
        408 | 504 => ProviderError::timeout("image_proxy", message),
        429 => ProviderError::rate_limit_with_retry("image_proxy", message, None),
        502 | 503 => ProviderError::provider_unavailable("image_proxy", message),
        _ => ProviderError::api_error("image_proxy", status, message),
    }
}

fn image_proxy_tokens_used(usage: &PricingUsage) -> u64 {
    u64::from(
        usage
            .total_tokens
            .saturating_add(usage.image_tokens.unwrap_or(0)),
    )
}

fn image_provider_supports_requested_model(
    provider: &ProviderConfig,
    requested_model: Option<&str>,
) -> bool {
    let Some(requested_model) = requested_model else {
        return true;
    };
    provider.models.is_empty() || provider.models.iter().any(|model| model == requested_model)
}

fn image_proxy_base_url(provider: &ProviderConfig) -> Result<String, GatewayError> {
    if let Some(base_url) = provider.base_url.as_deref() {
        let trimmed = base_url.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if provider_config::normalize_provider_selector(&provider.provider_type) == "openai"
        || provider_config::normalize_provider_selector(&provider.name) == "openai"
    {
        return Ok(OPENAI_IMAGE_BASE_URL.to_string());
    }

    Err(GatewayError::Config(format!(
        "Image provider '{}' is missing base_url",
        provider.name
    )))
}

fn image_proxy_url(
    provider: &ImageProxyProvider,
    endpoint: ImageProxyEndpoint,
) -> Result<Url, GatewayError> {
    let mut url = Url::parse(&provider.base_url)
        .map_err(|error| GatewayError::Config(format!("Invalid image provider URL: {error}")))?;
    url.path_segments_mut()
        .map_err(|_| GatewayError::Config("Invalid image provider URL".to_string()))?
        .extend(["images", endpoint.path_segment()]);
    Ok(url)
}

fn image_multipart_content_type(req: &HttpRequest) -> Result<String, GatewayError> {
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .ok_or_else(|| GatewayError::validation("multipart/form-data content type is required"))?
        .to_str()
        .map_err(|_| GatewayError::validation("Invalid content type"))?
        .trim()
        .to_string();

    if content_type
        .to_ascii_lowercase()
        .starts_with("multipart/form-data")
    {
        return Ok(content_type);
    }

    Err(GatewayError::validation(
        "multipart/form-data content type is required",
    ))
}

fn missing_image_proxy_provider_error() -> GatewayError {
    GatewayError::BadRequest(
        "Image edits and variations API requires an enabled openai or openai_compatible provider"
            .to_string(),
    )
}

fn apply_image_proxy_headers(
    mut request: ProviderRequestBuilder,
    provider: &ImageProxyProvider,
) -> ProviderRequestBuilder {
    for (name, value) in &provider.headers {
        request = request.header(name.clone(), value.clone());
    }
    request
}

fn image_proxy_provider_headers(
    provider: &ProviderConfig,
) -> Result<Vec<(HeaderName, HeaderValue)>, GatewayError> {
    let mut headers = Vec::new();
    push_image_proxy_header(
        &mut headers,
        "Authorization",
        format!("Bearer {}", provider.api_key),
    )?;
    if let Some(organization) = provider.organization.as_deref() {
        push_image_proxy_header(&mut headers, "OpenAI-Organization", organization)?;
    }
    if let Some(project) = provider.project.as_deref() {
        push_image_proxy_header(&mut headers, "OpenAI-Project", project)?;
    }
    provider_config::append_string_header_map(provider, "headers", |key, value| {
        push_image_proxy_header(&mut headers, key, value)
    })?;
    provider_config::append_string_header_map(provider, "custom_headers", |key, value| {
        push_image_proxy_header(&mut headers, key, value)
    })?;
    Ok(headers)
}

fn push_image_proxy_header(
    headers: &mut Vec<(HeaderName, HeaderValue)>,
    name: impl AsRef<str>,
    value: impl AsRef<str>,
) -> Result<(), GatewayError> {
    let name = HeaderName::from_bytes(name.as_ref().as_bytes())
        .map_err(|error| GatewayError::Config(format!("Invalid image provider header: {error}")))?;
    let value = HeaderValue::from_str(value.as_ref()).map_err(|error| {
        GatewayError::Config(format!("Invalid image provider header value: {error}"))
    })?;
    headers.push((name, value));
    Ok(())
}

fn is_openai_image_provider(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

    provider_type == "openai"
        || provider_type == "openaicompatible"
        || provider_name == "openai"
        || provider_name == "openaicompatible"
}

impl ImageProxyEndpoint {
    fn capability(self) -> ProviderCapability {
        match self {
            Self::Edits => ProviderCapability::ImageEdit,
            Self::Variations => ProviderCapability::ImageVariation,
        }
    }

    fn path_segment(self) -> &'static str {
        match self {
            Self::Edits => "edits",
            Self::Variations => "variations",
        }
    }
}
