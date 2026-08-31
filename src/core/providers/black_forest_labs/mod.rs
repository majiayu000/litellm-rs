//! Native Black Forest Labs image generation and editing transport.

use std::collections::HashMap;
use std::pin::Pin;

use base64::Engine as _;
use futures::Stream;
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::core::net::validate_outbound_url_str;
use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HeaderPair, HttpErrorMapper, apply_provider_headers, header_owned,
};
use crate::core::providers::media::config_boundary::{MediaCredential, validate_media_config};
use crate::core::providers::media::{
    GenerationLifecycle, GenerationOutput, GenerationPoll, PollPolicy,
};
use crate::core::providers::{LLMProvider, ProviderError};
use crate::core::traits::error_mapper::{DefaultErrorMapper, trait_def::ErrorMapper};
use crate::core::traits::provider::ProviderConfig;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::health::HealthStatus;
use crate::core::types::image::{ImageEditRequest, ImageGenerationRequest};
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::{ChatChunk, ChatResponse, ImageData, ImageGenerationResponse};

const PROVIDER: &str = "black_forest_labs";
const DEFAULT_API_BASE: &str = "https://api.bfl.ai/v1";
const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ImageGeneration,
    ProviderCapability::ImageEdit,
];
const GENERATION_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::ImageGeneration];
const KONTEXT_MODELS: &[&str] = &["flux-kontext-pro", "flux-kontext-max"];
const MODELS: &[&str] = &[
    "flux-pro-1.1",
    "flux-pro-1.1-ultra",
    "flux-dev",
    "flux-kontext-pro",
    "flux-kontext-max",
];

/// Black Forest Labs provider configuration.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct BflConfig {
    #[serde(flatten)]
    pub base: BaseConfig,
    #[serde(skip, default)]
    pub poll_policy: PollPolicy,
}

impl std::fmt::Debug for BflConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BflConfig")
            .field("endpoint_access", &self.base.endpoint_access)
            .field("has_api_key", &self.base.api_key.is_some())
            .field("custom_header_count", &self.base.headers.len())
            .finish()
    }
}

impl Default for BflConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_base: Some(DEFAULT_API_BASE.to_string()),
                ..BaseConfig::default()
            },
            poll_policy: PollPolicy::default(),
        }
    }
}

impl BflConfig {
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_key = Some(api_key.into());
        config
    }

    pub fn from_env() -> Self {
        let mut config = Self::default();
        let env = BaseConfig::from_env("bfl");
        config.base.api_key = env.api_key;
        config.base.timeout = env.timeout;
        config.base.max_retries = env.max_retries;
        if env.api_base.is_some() {
            config.base.api_base = env.api_base;
        }
        config
    }
}

impl ProviderConfig for BflConfig {
    fn validate(&self) -> Result<(), String> {
        self.base.validate("bfl")
    }

    fn api_key(&self) -> Option<&str> {
        self.base.api_key.as_deref()
    }

    fn api_base(&self) -> Option<&str> {
        self.base.api_base.as_deref()
    }

    fn timeout(&self) -> std::time::Duration {
        self.base.timeout_duration()
    }

    fn max_retries(&self) -> u32 {
        self.base.max_retries
    }
}

/// Provider-native BFL payload. Parameters are forwarded without renaming.
#[derive(Debug, Clone)]
pub struct BflImageRequest {
    pub model: String,
    pub parameters: Map<String, Value>,
}

impl BflImageRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        let mut parameters = Map::new();
        parameters.insert("prompt".to_string(), Value::String(prompt.into()));
        Self {
            model: model.into(),
            parameters,
        }
    }
}

/// Native BFL provider.
#[derive(Clone)]
pub struct BflProvider {
    config: BflConfig,
    client: BaseHttpClient,
    lifecycle: GenerationLifecycle,
    models: Vec<ModelInfo>,
}

impl BflProvider {
    pub fn new(mut config: BflConfig) -> Result<Self, ProviderError> {
        validate_media_config(PROVIDER, &mut config.base, MediaCredential::Raw, &["x-key"])?;
        config
            .validate()
            .map_err(|error| ProviderError::configuration(PROVIDER, error))?;
        Ok(Self {
            client: BaseHttpClient::new_for_provider_no_redirect(PROVIDER, config.base.clone())?,
            lifecycle: GenerationLifecycle::new_no_redirect(
                PROVIDER,
                config.base.clone(),
                config.poll_policy,
            )?,
            config,
            models: supported_models(),
        })
    }

    pub fn from_env() -> Result<Self, ProviderError> {
        Self::new(BflConfig::from_env())
    }

    fn request_headers(&self, api_key: &str) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(self.config.base.headers.len() + 1);
        for (key, value) in &self.config.base.headers {
            if key.eq_ignore_ascii_case("x-key") {
                continue;
            }
            headers.push(header_owned(key.clone(), value.clone()));
        }
        headers.push(header_owned("x-key".to_string(), api_key.to_string()));
        headers
    }

    /// Submit and await a provider-native BFL image request.
    pub async fn generate_native(
        &self,
        request: BflImageRequest,
        cancellation: &CancellationToken,
    ) -> Result<GenerationOutput, ProviderError> {
        if !MODELS.contains(&request.model.as_str()) {
            return Err(ProviderError::model_not_found(PROVIDER, request.model));
        }
        self.validate_webhook(&request.parameters).await?;
        let api_key = self.api_key()?;
        let url = format!(
            "{}/{}",
            self.config
                .base
                .api_base
                .as_deref()
                .unwrap_or(DEFAULT_API_BASE)
                .trim_end_matches('/'),
            request.model
        );
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let headers = self.request_headers(api_key);
        let submit = apply_provider_headers(self.client.post(url)?, headers.clone())
            .json(&request.parameters);
        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(cancelled()),
            response = submit.send() => response,
        }
        .map_err(|error| {
            let may_have_been_dispatched =
                BaseHttpClient::request_error_may_have_been_dispatched(&error);
            let error = self.client.map_preserved_request_error(error);
            if may_have_been_dispatched {
                mark_post_submit_error_non_retryable(error)
            } else {
                error
            }
        })?;
        let status = response.status();
        let body = tokio::select! {
            _ = cancellation.cancelled() => return Err(cancelled()),
            body = response.text() => body,
        };
        let body = match body {
            Ok(body) => body,
            Err(error) => {
                return Err(if status.is_success() {
                    mark_post_submit_error_non_retryable(
                        self.client.map_preserved_request_error(error),
                    )
                } else {
                    HttpErrorMapper::map_status_without_body(PROVIDER, status.as_u16())
                });
            }
        };
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                PROVIDER,
                status.as_u16(),
                &body,
            ));
        }
        let submit: Value = serde_json::from_str(&body)
            .map_err(|error| {
                ProviderError::response_parsing(
                    PROVIDER,
                    format!("invalid BFL submit response: {error}"),
                )
            })
            .map_err(mark_post_submit_error_non_retryable)?;
        let polling_url = submit["polling_url"]
            .as_str()
            .ok_or_else(|| {
                ProviderError::response_parsing(PROVIDER, "BFL response omitted polling_url")
            })
            .map_err(mark_post_submit_error_non_retryable)?;
        validate_polling_origin(
            self.config
                .base
                .api_base
                .as_deref()
                .unwrap_or(DEFAULT_API_BASE),
            polling_url,
        )
        .map_err(mark_post_submit_error_non_retryable)?;
        self.lifecycle
            .wait_for_json(polling_url.to_string(), headers, cancellation, decode_poll)
            .await
            .map_err(mark_post_submit_error_non_retryable)
    }

    async fn validate_webhook(&self, parameters: &Map<String, Value>) -> Result<(), ProviderError> {
        let Some(webhook_url) = parameters.get("webhook_url").and_then(Value::as_str) else {
            return Ok(());
        };
        let webhook_url = webhook_url.to_string();
        tokio::task::spawn_blocking(move || validate_outbound_url_str(&webhook_url))
            .await
            .map_err(|error| ProviderError::api_error(PROVIDER, 500, error.to_string()))?
            .map(|_| ())
            .map_err(|error| ProviderError::invalid_request(PROVIDER, error.to_string()))
    }

    fn api_key(&self) -> Result<&str, ProviderError> {
        self.config
            .base
            .api_key
            .as_deref()
            .ok_or_else(|| ProviderError::authentication(PROVIDER, "API key is required"))
    }

    async fn unified_generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let model = request.model.as_deref().unwrap_or("flux-pro-1.1");
        self.validate_image_generation_request(&request, model)?;
        let mut native = BflImageRequest::new(model, request.prompt);
        if let Some(size) = request.size {
            insert_size_parameters(model, &size, &mut native.parameters)?;
        }
        self.to_image_response(
            self.generate_native(native, &CancellationToken::new())
                .await?,
        )
    }

    pub(crate) fn validate_image_generation_request(
        &self,
        request: &ImageGenerationRequest,
        model: &str,
    ) -> Result<(), ProviderError> {
        if !MODELS.contains(&model) {
            return Err(ProviderError::model_not_found(PROVIDER, model));
        }
        if request.n.is_some_and(|count| count != 1) {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "BFL native generation returns exactly one image",
            ));
        }
        if request.quality.is_some() || request.style.is_some() {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "BFL does not accept OpenAI quality or style parameters",
            ));
        }
        if !matches!(request.response_format.as_deref(), None | Some("url")) {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "BFL native results are returned as signed URLs",
            ));
        }
        if let Some(size) = request.size.as_deref() {
            insert_size_parameters(model, size, &mut Map::new())?;
        }
        Ok(())
    }

    async fn unified_edit(
        &self,
        request: ImageEditRequest,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let model = request.model.as_deref().unwrap_or("flux-kontext-pro");
        self.validate_image_edit_request(&request, model)?;
        let mut native = BflImageRequest::new(model, request.prompt);
        if let Some(size) = request.size {
            insert_size_parameters(model, &size, &mut native.parameters)?;
        }
        native.parameters.insert(
            "input_image".to_string(),
            Value::String(base64::engine::general_purpose::STANDARD.encode(request.image)),
        );
        self.to_image_response(
            self.generate_native(native, &CancellationToken::new())
                .await?,
        )
    }

    pub(crate) fn validate_image_edit_request(
        &self,
        request: &ImageEditRequest,
        model: &str,
    ) -> Result<(), ProviderError> {
        if request.n.is_some_and(|count| count != 1) {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "BFL native editing returns exactly one image",
            ));
        }
        if request.mask.is_some() {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "BFL Kontext editing does not accept a mask",
            ));
        }
        if !matches!(request.response_format.as_deref(), None | Some("url")) {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "BFL native results are returned as signed URLs",
            ));
        }
        if !KONTEXT_MODELS.contains(&model) {
            return Err(ProviderError::not_supported(
                PROVIDER,
                format!("image_edit for model '{model}'"),
            ));
        }
        if let Some(size) = request.size.as_deref() {
            insert_size_parameters(model, size, &mut Map::new())?;
        }
        Ok(())
    }

    fn to_image_response(
        &self,
        output: GenerationOutput,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let url = output.urls.into_iter().next().ok_or_else(|| {
            ProviderError::response_parsing(PROVIDER, "BFL result omitted sample URL")
        })?;
        Ok(ImageGenerationResponse {
            created: chrono::Utc::now().timestamp().unsigned_abs(),
            data: vec![ImageData {
                url: Some(url),
                b64_json: None,
                revised_prompt: None,
            }],
        })
    }
}

impl std::fmt::Debug for BflProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BflProvider")
            .field("provider", &PROVIDER)
            .field("model_count", &self.models.len())
            .finish()
    }
}

fn validate_polling_origin(api_base: &str, polling_url: &str) -> Result<(), ProviderError> {
    let api_base = url::Url::parse(api_base).map_err(|error| {
        ProviderError::response_parsing(PROVIDER, format!("invalid BFL API base: {error}"))
    })?;
    let polling_url = url::Url::parse(polling_url).map_err(|error| {
        ProviderError::response_parsing(PROVIDER, format!("invalid BFL polling_url: {error}"))
    })?;
    let same_origin = api_base.scheme() == polling_url.scheme()
        && api_base.host_str() == polling_url.host_str()
        && api_base.port_or_known_default() == polling_url.port_or_known_default();
    if !same_origin {
        return Err(ProviderError::response_parsing(
            PROVIDER,
            "BFL polling_url must use the configured API origin",
        ));
    }
    Ok(())
}

fn decode_poll(payload: Value) -> Result<GenerationPoll, ProviderError> {
    match payload["status"].as_str() {
        Some("Ready") => {
            let sample = payload["result"]["sample"].as_str().ok_or_else(|| {
                ProviderError::response_parsing(
                    PROVIDER,
                    "BFL Ready response omitted result.sample",
                )
            })?;
            let parsed = url::Url::parse(sample).map_err(|_| {
                ProviderError::response_parsing(
                    PROVIDER,
                    "BFL result.sample must be an HTTP(S) URL",
                )
            })?;
            if sample.is_empty() || !matches!(parsed.scheme(), "http" | "https") {
                return Err(ProviderError::response_parsing(
                    PROVIDER,
                    "BFL result.sample must be an HTTP(S) URL",
                ));
            }
            Ok(GenerationPoll::Succeeded(GenerationOutput {
                urls: vec![sample.to_string()],
                credits_used: payload["cost"].as_f64(),
            }))
        }
        Some("Error" | "Failed") => Ok(GenerationPoll::Failed(
            payload["error"]
                .as_str()
                .unwrap_or("BFL generation failed")
                .to_string(),
        )),
        Some("Request Moderated") => Err(ProviderError::content_filtered(
            PROVIDER,
            "BFL generation was moderated",
            None,
            Some(false),
        )),
        Some("Pending" | "Queued" | "Processing" | "Running") => Ok(GenerationPoll::Pending),
        status => Err(ProviderError::response_parsing(
            PROVIDER,
            format!("unknown BFL task status: {status:?}"),
        )),
    }
}

fn mark_post_submit_error_non_retryable(error: ProviderError) -> ProviderError {
    match error {
        ProviderError::ContentFiltered { .. } | ProviderError::Cancelled { .. } => error,
        error => ProviderError::other(
            PROVIDER,
            format!(
                "BFL task was already accepted; polling failed and resubmitting could duplicate it: {error}"
            ),
        ),
    }
}

pub(crate) fn is_post_submit_error(error: &ProviderError) -> bool {
    matches!(error, ProviderError::Other { provider: PROVIDER, message }
        if message.starts_with("BFL task was already accepted;"))
}

fn parse_size(size: &str) -> Result<(u32, u32), ProviderError> {
    let (width, height) = size.split_once('x').ok_or_else(|| {
        ProviderError::invalid_request(PROVIDER, format!("invalid image size '{size}'"))
    })?;
    let width = width.parse().map_err(|_| {
        ProviderError::invalid_request(PROVIDER, format!("invalid image size '{size}'"))
    })?;
    let height = height.parse().map_err(|_| {
        ProviderError::invalid_request(PROVIDER, format!("invalid image size '{size}'"))
    })?;
    if width == 0 || height == 0 {
        return Err(ProviderError::invalid_request(
            PROVIDER,
            format!("invalid image size '{size}'"),
        ));
    }
    Ok((width, height))
}

fn insert_size_parameters(
    model: &str,
    size: &str,
    parameters: &mut Map<String, Value>,
) -> Result<(), ProviderError> {
    let (width, height) = parse_size(size)?;
    if model == "flux-pro-1.1-ultra" {
        return Err(ProviderError::invalid_request(
            PROVIDER,
            format!("BFL model '{model}' cannot guarantee exact image size '{size}'"),
        ));
    }
    if KONTEXT_MODELS.contains(&model) {
        if (width, height) != (1024, 1024) {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                format!("BFL model '{model}' cannot guarantee exact image size '{size}'"),
            ));
        }
        parameters.insert("aspect_ratio".to_string(), Value::String("1:1".to_string()));
    } else {
        parameters.insert("width".to_string(), json!(width));
        parameters.insert("height".to_string(), json!(height));
    }
    Ok(())
}

fn cancelled() -> ProviderError {
    ProviderError::cancelled(
        PROVIDER,
        "media generation",
        Some("cancellation requested".to_string()),
    )
}

impl LLMProvider for BflProvider {
    fn name(&self) -> &'static str {
        PROVIDER
    }

    fn error_provider_name(&self) -> &'static str {
        PROVIDER
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["model", "n", "size", "response_format"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        Ok(params)
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        Err(ProviderError::not_supported(PROVIDER, "chat_completion"))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported(PROVIDER, "chat_completion"))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported(PROVIDER, "chat_completion"))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        Err(ProviderError::not_supported(PROVIDER, "streaming"))
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.unified_generate(request).await
    }

    async fn image_edit(
        &self,
        request: ImageEditRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.unified_edit(request).await
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Unknown
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        Err(ProviderError::not_supported(
            PROVIDER,
            "token pricing is unavailable for per-image/per-credit BFL models",
        ))
    }
}

fn supported_models() -> Vec<ModelInfo> {
    MODELS
        .iter()
        .map(|model| ModelInfo {
            id: (*model).to_string(),
            name: (*model).to_string(),
            provider: PROVIDER.to_string(),
            max_context_length: 0,
            supports_multimodal: true,
            capabilities: if KONTEXT_MODELS.contains(model) {
                CAPABILITIES.to_vec()
            } else {
                GENERATION_CAPABILITIES.to_vec()
            },
            ..ModelInfo::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_poll_requires_an_http_result_url() {
        let error = decode_poll(json!({
            "status": "Ready",
            "result": { "sample": "file:///tmp/result.png" }
        }))
        .expect_err("non-HTTP output must not escape as a successful result");

        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    }

    #[test]
    fn moderated_poll_is_content_filtered() {
        let error = decode_poll(json!({ "status": "Request Moderated" }))
            .expect_err("moderation must retain its content-policy direction");

        assert!(matches!(error, ProviderError::ContentFiltered { .. }));
    }
}
