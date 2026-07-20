//! Fine-tuning API route handlers.
//!
//! These endpoints expose OpenAI-compatible fine-tuning lifecycle routes and
//! proxy them to an enabled OpenAI or OpenAI-compatible provider.

use crate::config::models::provider::ProviderConfig;
use crate::core::fine_tuning::config::ProviderFineTuningConfig;
use crate::core::fine_tuning::providers::{
    FineTuningError, FineTuningProvider, OpenAIFineTuningProvider,
};
use crate::core::fine_tuning::types::{
    CreateJobRequest, FineTuningCheckpoint, ListEventsParams, ListJobsParams,
};
use crate::core::router::execution::is_retryable_error;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpResponse, Result as ActixResult, web};
use reqwest::header::{HeaderName, HeaderValue};
use serde::Serialize;
use std::collections::HashMap;
use std::future::Future;
use tracing::error;

use super::budgeted::SettlementMode;
use super::openai_errors;
use super::provider_config;

const OPENAI_FINE_TUNING_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone)]
struct FineTuningRouteProvider {
    name: String,
    config: ProviderFineTuningConfig,
}

enum FineTuningRouteError {
    Gateway(GatewayError),
    FineTuning(FineTuningError),
}

#[derive(Serialize)]
struct ListCheckpointsResponse {
    object: &'static str,
    data: Vec<FineTuningCheckpoint>,
}

/// Create a fine-tuning job.
pub async fn create_fine_tuning_job(
    state: web::Data<AppState>,
    request: web::Json<CreateJobRequest>,
) -> ActixResult<HttpResponse> {
    let request = request.into_inner();
    if let Err(error) = validate_create_job_request(&request) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let model = request.model.clone();
    match execute_fine_tuning_route(state.get_ref(), &model, move |provider| {
        let request = request.clone();
        async move { provider.create_job(request).await }
    })
    .await
    {
        Ok(job) => Ok(HttpResponse::Ok().json(job)),
        Err(error) => {
            error!(
                "Fine-tuning create error: {}",
                fine_tuning_route_error_message(&error)
            );
            Ok(fine_tuning_route_error_response(error))
        }
    }
}

/// List fine-tuning jobs.
pub async fn list_fine_tuning_jobs(
    state: web::Data<AppState>,
    query: web::Query<ListJobsParams>,
) -> ActixResult<HttpResponse> {
    let query = query.into_inner();
    match execute_fine_tuning_route(state.get_ref(), "", move |provider| {
        let query = query.clone();
        async move { provider.list_jobs(query).await }
    })
    .await
    {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(error) => {
            error!(
                "Fine-tuning list error: {}",
                fine_tuning_route_error_message(&error)
            );
            Ok(fine_tuning_route_error_response(error))
        }
    }
}

/// Get a fine-tuning job by ID.
pub async fn get_fine_tuning_job(
    state: web::Data<AppState>,
    job_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let job_id = job_id.into_inner();
    if let Err(error) = validate_fine_tuning_job_id(&job_id) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    match execute_fine_tuning_route(state.get_ref(), "", move |provider| {
        let job_id = job_id.clone();
        async move { provider.get_job(&job_id).await }
    })
    .await
    {
        Ok(job) => Ok(HttpResponse::Ok().json(job)),
        Err(error) => {
            error!(
                "Fine-tuning retrieve error: {}",
                fine_tuning_route_error_message(&error)
            );
            Ok(fine_tuning_route_error_response(error))
        }
    }
}

/// Cancel a fine-tuning job by ID.
pub async fn cancel_fine_tuning_job(
    state: web::Data<AppState>,
    job_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let job_id = job_id.into_inner();
    if let Err(error) = validate_fine_tuning_job_id(&job_id) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    match execute_fine_tuning_route(state.get_ref(), "", move |provider| {
        let job_id = job_id.clone();
        async move { provider.cancel_job(&job_id).await }
    })
    .await
    {
        Ok(job) => Ok(HttpResponse::Ok().json(job)),
        Err(error) => {
            error!(
                "Fine-tuning cancel error: {}",
                fine_tuning_route_error_message(&error)
            );
            Ok(fine_tuning_route_error_response(error))
        }
    }
}

/// List events for a fine-tuning job.
pub async fn list_fine_tuning_events(
    state: web::Data<AppState>,
    job_id: web::Path<String>,
    query: web::Query<ListEventsParams>,
) -> ActixResult<HttpResponse> {
    let job_id = job_id.into_inner();
    if let Err(error) = validate_fine_tuning_job_id(&job_id) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let query = query.into_inner();
    match execute_fine_tuning_route(state.get_ref(), "", move |provider| {
        let job_id = job_id.clone();
        let query = query.clone();
        async move { provider.list_events(&job_id, query).await }
    })
    .await
    {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(error) => {
            error!(
                "Fine-tuning events error: {}",
                fine_tuning_route_error_message(&error)
            );
            Ok(fine_tuning_route_error_response(error))
        }
    }
}

/// List checkpoints for a fine-tuning job.
pub async fn list_fine_tuning_checkpoints(
    state: web::Data<AppState>,
    job_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let job_id = job_id.into_inner();
    if let Err(error) = validate_fine_tuning_job_id(&job_id) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    match execute_fine_tuning_route(state.get_ref(), "", move |provider| {
        let job_id = job_id.clone();
        async move { provider.list_checkpoints(&job_id).await }
    })
    .await
    {
        Ok(data) => Ok(HttpResponse::Ok().json(ListCheckpointsResponse {
            object: "list",
            data,
        })),
        Err(error) => {
            error!(
                "Fine-tuning checkpoints error: {}",
                fine_tuning_route_error_message(&error)
            );
            Ok(fine_tuning_route_error_response(error))
        }
    }
}

async fn execute_fine_tuning_route<T, F, Fut>(
    state: &AppState,
    model: &str,
    operation: F,
) -> Result<T, FineTuningRouteError>
where
    F: Fn(OpenAIFineTuningProvider) -> Fut,
    Fut: Future<Output = Result<T, FineTuningError>>,
{
    let config = state.config();
    let provider_configs =
        select_fine_tuning_route_provider_configs(config.gateway.providers.as_slice());
    if provider_configs.is_empty() {
        return Err(FineTuningRouteError::Gateway(
            missing_fine_tuning_provider_error(),
        ));
    }

    let provider_count = provider_configs.len();
    let mut last_error = None;
    for (index, provider_config) in provider_configs.into_iter().enumerate() {
        let is_last_provider = index + 1 == provider_count;
        if let Err(error) = ensure_fine_tuning_budget(state, &provider_config.name, model) {
            let error = FineTuningRouteError::Gateway(error);
            if !is_last_provider && is_retryable_fine_tuning_route_error(&error) {
                last_error = Some(error);
                continue;
            }
            return Err(error);
        }

        let route_provider =
            fine_tuning_route_provider(provider_config).map_err(FineTuningRouteError::Gateway)?;
        let provider =
            OpenAIFineTuningProvider::new_named(route_provider.config, route_provider.name)
                .map_err(FineTuningRouteError::FineTuning)?;
        match operation(provider).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                let error = FineTuningRouteError::FineTuning(error);
                if !is_last_provider && is_retryable_fine_tuning_route_error(&error) {
                    last_error = Some(error);
                    continue;
                }
                return Err(error);
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| FineTuningRouteError::Gateway(missing_fine_tuning_provider_error())))
}

fn ensure_fine_tuning_budget(
    state: &AppState,
    provider: &str,
    model: &str,
) -> Result<(), GatewayError> {
    state
        .budgeted
        .for_selected(provider, model)
        .with_settlement_mode(SettlementMode::AvailabilityOnly)
        .ensure_available()
        .map_err(GatewayError::from)
}

fn is_retryable_fine_tuning_route_error(error: &FineTuningRouteError) -> bool {
    match error {
        FineTuningRouteError::Gateway(GatewayError::Provider(error)) => is_retryable_error(error),
        FineTuningRouteError::Gateway(
            GatewayError::Network(_)
            | GatewayError::Timeout(_)
            | GatewayError::Unavailable(_)
            | GatewayError::RateLimit { .. },
        ) => true,
        FineTuningRouteError::FineTuning(error) => is_retryable_fine_tuning_error(error),
        _ => false,
    }
}

fn is_retryable_fine_tuning_error(error: &FineTuningError) -> bool {
    match error {
        FineTuningError::RateLimited { .. } | FineTuningError::Network { .. } => true,
        FineTuningError::Provider { message } => {
            let Some(status) = message
                .strip_prefix("API error ")
                .and_then(|tail| tail.split(':').next())
                .and_then(|status| status.split_whitespace().next())
                .and_then(|status| status.parse::<u16>().ok())
            else {
                return false;
            };
            matches!(status, 408 | 429 | 500 | 502 | 503 | 504 | 507 | 529)
        }
        _ => false,
    }
}

#[cfg(test)]
fn select_fine_tuning_route_providers(
    providers: &[ProviderConfig],
) -> Result<Vec<FineTuningRouteProvider>, GatewayError> {
    select_fine_tuning_route_provider_configs(providers)
        .into_iter()
        .map(fine_tuning_route_provider)
        .collect()
}

fn select_fine_tuning_route_provider_configs(providers: &[ProviderConfig]) -> Vec<&ProviderConfig> {
    providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_openai_fine_tuning_provider(provider))
        .collect()
}

fn fine_tuning_route_provider(
    provider: &ProviderConfig,
) -> Result<FineTuningRouteProvider, GatewayError> {
    if provider.api_key.trim().is_empty() {
        return Err(GatewayError::Config(format!(
            "Fine-tuning provider '{}' is missing api_key",
            provider.name
        )));
    }

    Ok(FineTuningRouteProvider {
        name: provider.name.clone(),
        config: ProviderFineTuningConfig {
            enabled: true,
            api_key: Some(provider.api_key.clone()),
            api_base: Some(fine_tuning_api_base(provider)?),
            endpoint_access: provider.endpoint_access,
            organization_id: provider.organization.clone(),
            supported_models: provider.model_groups(),
            timeout_seconds: provider.timeout,
            headers: fine_tuning_provider_headers(provider)?,
        },
    })
}

fn fine_tuning_api_base(provider: &ProviderConfig) -> Result<String, GatewayError> {
    if let Some(base_url) = provider.base_url.as_deref() {
        let trimmed = base_url.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    if provider_config::normalize_provider_selector(&provider.provider_type) == "openai"
        || provider_config::normalize_provider_selector(&provider.name) == "openai"
    {
        return Ok(OPENAI_FINE_TUNING_BASE_URL.to_string());
    }

    Err(GatewayError::Config(format!(
        "Fine-tuning provider '{}' is missing base_url",
        provider.name
    )))
}

fn fine_tuning_provider_headers(
    provider: &ProviderConfig,
) -> Result<HashMap<String, String>, GatewayError> {
    let mut headers = HashMap::new();
    if let Some(project) = provider.project.as_deref() {
        push_config_header(&mut headers, "OpenAI-Project", project)?;
    }
    provider_config::append_string_header_map(provider, "headers", |key, value| {
        push_config_header(&mut headers, key, value)
    })?;
    provider_config::append_string_header_map(provider, "custom_headers", |key, value| {
        push_config_header(&mut headers, key, value)
    })?;
    Ok(headers)
}

fn push_config_header(
    headers: &mut HashMap<String, String>,
    name: impl AsRef<str>,
    value: impl AsRef<str>,
) -> Result<(), GatewayError> {
    HeaderName::from_bytes(name.as_ref().as_bytes()).map_err(|error| {
        GatewayError::Config(format!("Invalid fine-tuning provider header: {error}"))
    })?;
    HeaderValue::from_str(value.as_ref()).map_err(|error| {
        GatewayError::Config(format!(
            "Invalid fine-tuning provider header value: {error}"
        ))
    })?;
    headers.insert(name.as_ref().to_string(), value.as_ref().to_string());
    Ok(())
}

fn validate_create_job_request(request: &CreateJobRequest) -> Result<(), GatewayError> {
    if request.model.trim().is_empty() {
        return Err(GatewayError::validation("model is required"));
    }
    if request.training_file.trim().is_empty() {
        return Err(GatewayError::validation("training_file is required"));
    }
    Ok(())
}

fn validate_fine_tuning_job_id(job_id: &str) -> Result<(), GatewayError> {
    if job_id.trim().is_empty() {
        return Err(GatewayError::validation("fine_tuning_job_id is required"));
    }

    if job_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Ok(());
    }

    Err(GatewayError::validation(
        "fine_tuning_job_id must be a single safe path segment",
    ))
}

fn missing_fine_tuning_provider_error() -> GatewayError {
    GatewayError::Config(
        "Fine-tuning API requires an enabled openai or openai_compatible provider".to_string(),
    )
}

fn fine_tuning_route_error_message(error: &FineTuningRouteError) -> String {
    match error {
        FineTuningRouteError::Gateway(error) => error.to_string(),
        FineTuningRouteError::FineTuning(error) => error.to_string(),
    }
}

fn fine_tuning_route_error_response(error: FineTuningRouteError) -> HttpResponse {
    match error {
        FineTuningRouteError::Gateway(error) => openai_errors::gateway_error_response(&error),
        FineTuningRouteError::FineTuning(error) => fine_tuning_error_response(error),
    }
}

fn fine_tuning_error_response(error: FineTuningError) -> HttpResponse {
    match error {
        FineTuningError::Authentication { message } => openai_errors::unauthorized_error(message),
        FineTuningError::InvalidRequest { message } => openai_errors::validation_error(message),
        FineTuningError::JobNotFound { job_id } => {
            let gateway_error =
                GatewayError::NotFound(format!("Fine-tuning job not found: {job_id}"));
            openai_errors::gateway_error_response(&gateway_error)
        }
        FineTuningError::ProviderNotFound { provider } => {
            let gateway_error =
                GatewayError::Config(format!("Fine-tuning provider not found: {provider}"));
            openai_errors::gateway_error_response(&gateway_error)
        }
        FineTuningError::RateLimited {
            retry_after_seconds,
        } => {
            let gateway_error = GatewayError::RateLimit {
                message: "Fine-tuning provider rate limit exceeded".to_string(),
                retry_after: Some(retry_after_seconds),
                rpm_limit: None,
                tpm_limit: None,
            };
            openai_errors::gateway_error_response(&gateway_error)
        }
        FineTuningError::Network { message } => {
            let gateway_error = GatewayError::Network(message);
            openai_errors::gateway_error_response(&gateway_error)
        }
        FineTuningError::Provider { message } | FineTuningError::Other { message } => {
            let gateway_error = GatewayError::Unavailable(message);
            openai_errors::gateway_error_response(&gateway_error)
        }
    }
}

fn is_openai_fine_tuning_provider(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);

    provider_type == "openai"
        || provider_type == "openaicompatible"
        || provider_name == "openai"
        || provider_name == "openaicompatible"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn provider(name: &str, provider_type: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: provider_type.to_string(),
            api_key: "sk-test".to_string(),
            base_url: Some("https://fine-tuning.example.test/v1/".to_string()),
            ..ProviderConfig::default()
        }
    }

    #[test]
    fn selects_openai_compatible_fine_tuning_provider() {
        let selected =
            select_fine_tuning_route_providers(&[provider("primary", "openai_compatible")])
                .expect("provider selection should succeed");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "primary");
        assert_eq!(
            selected[0].config.api_base.as_deref(),
            Some("https://fine-tuning.example.test/v1")
        );
        assert_eq!(selected[0].config.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn selected_provider_preserves_project_and_custom_headers() {
        let mut provider = provider("mock-openai-compatible", "openai_compatible");
        provider.organization = Some("org-test".to_string());
        provider.project = Some("proj-test".to_string());
        provider.settings = HashMap::from([
            (
                "headers".to_string(),
                json!({
                    "X-Base-Header": "base-value",
                    "X-Ignored-Non-String": false
                }),
            ),
            (
                "custom_headers".to_string(),
                json!({
                    "X-Custom-Header": "custom-value"
                }),
            ),
        ]);

        let selected = select_fine_tuning_route_providers(&[provider])
            .expect("provider selection should succeed");

        assert_eq!(
            selected[0].config.organization_id.as_deref(),
            Some("org-test")
        );
        assert_eq!(selected[0].config.headers["OpenAI-Project"], "proj-test");
        assert_eq!(selected[0].config.headers["X-Base-Header"], "base-value");
        assert_eq!(
            selected[0].config.headers["X-Custom-Header"],
            "custom-value"
        );
        assert!(
            !selected[0]
                .config
                .headers
                .contains_key("X-Ignored-Non-String")
        );
    }

    #[test]
    fn compatible_provider_requires_base_url() {
        let mut provider = provider("local", "openai_compatible");
        provider.base_url = None;

        let error = select_fine_tuning_route_providers(&[provider]).unwrap_err();

        assert!(error.to_string().contains("missing base_url"));
    }

    #[test]
    fn validates_create_job_required_fields() {
        validate_create_job_request(&CreateJobRequest::new("gpt-4o-mini", "file-train"))
            .expect("request should validate");

        let mut request = CreateJobRequest::new("", "file-train");
        let error = validate_create_job_request(&request).unwrap_err();
        assert!(error.to_string().contains("model is required"));

        request.model = "gpt-4o-mini".to_string();
        request.training_file = " ".to_string();
        let error = validate_create_job_request(&request).unwrap_err();
        assert!(error.to_string().contains("training_file is required"));
    }

    #[test]
    fn rejects_unsafe_fine_tuning_job_id() {
        let error = validate_fine_tuning_job_id("../ftjob_123").unwrap_err();

        assert!(error.to_string().contains("safe path segment"));
    }

    #[test]
    fn retryable_provider_api_statuses_are_detected() {
        for status in [
            "408 Request Timeout",
            "429 Too Many Requests",
            "500 Internal Server Error",
            "502 Bad Gateway",
            "503 Service Unavailable",
            "504 Gateway Timeout",
            "507 Insufficient Storage",
            "529 Site is overloaded",
        ] {
            let error = FineTuningError::provider(format!("API error {status}: transient"));
            assert!(
                is_retryable_fine_tuning_error(&error),
                "{status} should be retryable"
            );
        }

        for status in ["400 Bad Request", "401 Unauthorized", "404 Not Found"] {
            let error = FineTuningError::provider(format!("API error {status}: client error"));
            assert!(
                !is_retryable_fine_tuning_error(&error),
                "{status} should not be retryable"
            );
        }

        let malformed = FineTuningError::provider("API error unavailable: malformed");
        assert!(!is_retryable_fine_tuning_error(&malformed));
    }
}
