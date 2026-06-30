//! Image API endpoints

use crate::config::models::provider::ProviderConfig;
use crate::core::models::openai::{ImageGenerationRequest, ImageGenerationResponse};
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::{Provider, ProviderError};
use crate::core::types::context::RequestContext;
use crate::core::types::image::ImageGenerationRequest as CoreImageRequest;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{
    HttpRequest, HttpResponse, Result as ActixResult, http::StatusCode, http::header, web,
};
use bytes::Bytes;
use futures::StreamExt;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::{Client, RequestBuilder, Url};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{error, info};

use super::context::handle_ai_request;
use super::execution::execute_with_selected_deployment;
use super::{openai_errors, provider_config};

const OPENAI_IMAGE_BASE_URL: &str = "https://api.openai.com/v1";
const MAX_IMAGE_MULTIPART_BYTES: usize = 64 * 1024 * 1024;
static IMAGE_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
enum ImageProxyEndpoint {
    Edits,
    Variations,
}

#[derive(Debug, Clone, PartialEq)]
struct ImageProxyProvider {
    provider_name: String,
    base_url: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    timeout: Duration,
}

#[derive(Debug, Clone)]
struct ImageProxyFormFields {
    model: Option<String>,
    prompt: Option<String>,
    size: Option<String>,
    quality: Option<String>,
    n: u32,
}

/// Image generation endpoint
///
/// OpenAI-compatible image generation API.
pub async fn image_generations(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ImageGenerationRequest>,
) -> ActixResult<HttpResponse> {
    info!("Image generation request for model: {:?}", request.model);

    handle_ai_request(
        &req,
        request.into_inner(),
        "Image generation",
        |request, context| handle_image_generation_with_state(state.get_ref(), request, context),
    )
    .await
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

/// Handle image generation with app state (UnifiedRouter only)
pub async fn handle_image_generation_with_state(
    state: &AppState,
    request: ImageGenerationRequest,
    context: RequestContext,
) -> Result<ImageGenerationResponse, GatewayError> {
    let unified_router = &state.unified_router;
    handle_image_generation_internal(unified_router, request, context).await
}

async fn handle_image_generation_internal(
    unified_router: &crate::core::router::UnifiedRouter,
    request: ImageGenerationRequest,
    context: RequestContext,
) -> Result<ImageGenerationResponse, GatewayError> {
    let requested_model = request
        .model
        .clone()
        .ok_or_else(|| GatewayError::validation("Model is required"))?;
    if requested_model.trim().is_empty() {
        return Err(GatewayError::validation("Model is required"));
    }

    let core_request = CoreImageRequest {
        prompt: request.prompt,
        model: Some(requested_model.clone()),
        n: request.n,
        size: request.size,
        response_format: request.response_format,
        user: request.user,
        quality: None,
        style: None,
    };

    let context_for_execution = context.clone();
    let core_response = execute_with_selected_deployment(
        unified_router,
        &requested_model,
        ProviderCapability::ImageGeneration,
        move |provider, selected_model, _deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            async move {
                let mut request_for_provider = core_request.clone();
                request_for_provider.model = Some(selected_model);
                let response = provider
                    .create_images(request_for_provider, context)
                    .await?;
                Ok((response, 0))
            }
        },
    )
    .await?;

    // Convert core response to OpenAI format
    let response = ImageGenerationResponse {
        created: core_response.created,
        data: core_response
            .data
            .into_iter()
            .map(|d| crate::core::models::openai::ImageObject {
                url: d.url,
                b64_json: d.b64_json,
            })
            .collect(),
    };

    Ok(response)
}

async fn proxy_image_multipart_endpoint(
    state: &AppState,
    req: &HttpRequest,
    payload: web::Payload,
    endpoint: ImageProxyEndpoint,
) -> Result<HttpResponse, GatewayError> {
    ensure_image_route_authorized(state, req)?;
    let content_type = image_multipart_content_type(req)?;
    let body = read_image_multipart_payload(payload).await?;
    let form_fields = extract_image_proxy_form_fields(&body, &content_type);
    let requested_model = required_image_proxy_model(&form_fields)?;
    ensure_image_proxy_candidate_configured(
        state.config().gateway.providers.as_slice(),
        requested_model,
    )?;
    let router_models =
        image_proxy_router_models(state.config().gateway.providers.as_slice(), requested_model);
    let usage = estimated_image_proxy_usage(&form_fields);
    let estimated_cost =
        image_proxy_cost(state.pricing.as_ref(), "openai", requested_model, &usage)?;
    if estimated_cost <= 0.0 {
        return Err(GatewayError::Config(format!(
            "Image model '{requested_model}' has non-positive pricing"
        )));
    }
    let api_key_id = super::context::get_authenticated_api_key(req).map(|key| key.metadata.id);

    let mut last_router_error = None;
    for router_model in router_models {
        let result = execute_with_selected_deployment(
            &state.unified_router,
            &router_model,
            endpoint.capability(),
            {
                let body = body.clone();
                let content_type = content_type.clone();
                let usage = usage.clone();
                move |selected_provider, selected_model, _deployment_id| {
                    let body = body.clone();
                    let content_type = content_type.clone();
                    let usage = usage.clone();
                    async move {
                        let provider = selected_image_proxy_provider(
                            state.config().gateway.providers.as_slice(),
                            &selected_provider,
                            &selected_model,
                            requested_model,
                        )?;
                        super::spend::ensure_budget_available(
                            &state.budget_limits,
                            &provider.provider_name,
                            requested_model,
                        )?;
                        let url = image_proxy_url(&provider, endpoint)
                            .map_err(image_proxy_gateway_error_to_provider_error)?;
                        let response =
                            apply_image_proxy_headers(image_http_client().post(url), &provider)
                                .header(reqwest::header::CONTENT_TYPE, content_type)
                                .body(body)
                                .timeout(provider.timeout)
                                .send()
                                .await
                                .map_err(|error| {
                                    ProviderError::network("image_proxy", error.to_string())
                                })?;

                        if !response.status().is_success() {
                            return Err(image_proxy_upstream_error(response).await);
                        }

                        record_image_proxy_spend(
                            state,
                            &provider,
                            requested_model,
                            &usage,
                            estimated_cost,
                            api_key_id,
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

async fn record_image_proxy_spend(
    state: &AppState,
    provider: &ImageProxyProvider,
    model: &str,
    usage: &PricingUsage,
    cost: f64,
    api_key_id: Option<uuid::Uuid>,
) {
    state
        .budget_limits
        .record_spend(&provider.provider_name, model, cost);
    if let Some(api_key_id) = api_key_id {
        let total_tokens = u64::from(
            usage
                .total_tokens
                .saturating_add(usage.image_tokens.unwrap_or(0)),
        );
        if let Err(error) = state
            .key_manager
            .record_usage(api_key_id, total_tokens, cost)
            .await
        {
            error!("failed to record image proxy usage for key {api_key_id}: {error}");
        }
    }
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

fn image_proxy_cost(
    pricing_service: &PricingService,
    provider: &str,
    model: &str,
    usage: &PricingUsage,
) -> Result<f64, GatewayError> {
    pricing_service
        .calculate_loaded_usage_cost_for_provider(provider, model, usage)
        .map(|breakdown| breakdown.total_cost)
}

fn estimated_image_proxy_usage(form_fields: &ImageProxyFormFields) -> PricingUsage {
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
    let mut usage = PricingUsage::new(prompt_tokens, image_tokens);
    usage.image_tokens = Some(image_tokens);
    usage
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
        Err(GatewayError::Config(format!(
            "Image provider for model '{requested_model}' is not configured"
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

    Ok(ImageProxyProvider {
        provider_name: provider.name.clone(),
        base_url: image_proxy_base_url(provider)?,
        headers: image_proxy_provider_headers(provider)?,
        timeout: Duration::from_secs(provider.timeout),
    })
}

fn image_proxy_gateway_error_to_provider_error(error: GatewayError) -> ProviderError {
    match error {
        GatewayError::Provider(error) => error,
        GatewayError::Validation(message) | GatewayError::BadRequest(message) => {
            ProviderError::invalid_request("image_proxy", message)
        }
        GatewayError::Config(message) => ProviderError::configuration("image_proxy", message),
        GatewayError::Auth(message) | GatewayError::Forbidden(message) => {
            ProviderError::authentication("image_proxy", message)
        }
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
        401 | 403 => ProviderError::authentication("image_proxy", message),
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

fn extract_multipart_text_field(
    body: &Bytes,
    content_type: &str,
    field_name: &str,
) -> Option<String> {
    let boundary = multipart_boundary(content_type)?;
    let marker = format!("--{boundary}");
    let body = String::from_utf8_lossy(body);

    for raw_part in body.split(&marker).skip(1) {
        if raw_part.starts_with("--") {
            continue;
        }
        let part = raw_part.trim_start_matches("\r\n");
        let Some((headers, value)) = part.split_once("\r\n\r\n") else {
            continue;
        };
        if !multipart_part_has_field_name(headers, field_name) {
            continue;
        }
        return Some(value.trim_end_matches("\r\n").trim().to_string());
    }

    None
}

fn multipart_boundary(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|segment| {
        let segment = segment.trim();
        let raw_boundary = segment.strip_prefix("boundary=")?;
        let boundary = raw_boundary.trim_matches('"').trim();
        (!boundary.is_empty()).then(|| boundary.to_string())
    })
}

fn multipart_part_has_field_name(headers: &str, field_name: &str) -> bool {
    headers.lines().any(|line| {
        let line = line.trim();
        line.to_ascii_lowercase()
            .starts_with("content-disposition:")
            && (line.contains(&format!("name=\"{field_name}\""))
                || line.contains(&format!("name={field_name}")))
    })
}

fn missing_image_proxy_provider_error() -> GatewayError {
    GatewayError::Config(
        "Image edits and variations API requires an enabled openai or openai_compatible provider"
            .to_string(),
    )
}

fn image_http_client() -> &'static Client {
    IMAGE_HTTP_CLIENT.get_or_init(Client::new)
}

fn apply_image_proxy_headers(
    mut request: RequestBuilder,
    provider: &ImageProxyProvider,
) -> RequestBuilder {
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
