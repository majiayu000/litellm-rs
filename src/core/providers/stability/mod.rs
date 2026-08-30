//! Native Stability AI image generation and editing transport.

use base64::Engine as _;
use futures::Stream;
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::ProviderError;
use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HeaderPair, HttpErrorMapper, apply_provider_headers, header,
    header_owned,
};
use crate::core::traits::error_mapper::{DefaultErrorMapper, trait_def::ErrorMapper};
use crate::core::traits::provider::ProviderConfig;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::health::HealthStatus;
use crate::core::types::image::{ImageEditRequest, ImageGenerationRequest};
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::{ChatChunk, ChatResponse, ImageData, ImageGenerationResponse};

const PROVIDER: &str = "stability";
const DEFAULT_API_BASE: &str = "https://api.stability.ai";
const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ImageGeneration,
    ProviderCapability::ImageEdit,
];
const GENERATION_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::ImageGeneration];
const EDIT_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::ImageEdit];
const GENERATION_MODELS: &[&str] = &[
    "stable-image-core",
    "stable-image-ultra",
    "sd3",
    "sd3-large",
    "sd3-large-turbo",
    "sd3-medium",
    "sd3.5-large",
    "sd3.5-large-turbo",
    "sd3.5-medium",
];

/// Stability AI provider configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StabilityConfig {
    #[serde(flatten)]
    pub base: BaseConfig,
}

impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            base: BaseConfig {
                api_base: Some(DEFAULT_API_BASE.to_string()),
                ..BaseConfig::default()
            },
        }
    }
}

impl StabilityConfig {
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        let mut config = Self::default();
        config.base.api_key = Some(api_key.into());
        config
    }

    pub fn from_env() -> Self {
        let mut config = Self::default();
        let env = BaseConfig::from_env(PROVIDER);
        config.base.api_key = env.api_key;
        config.base.timeout = env.timeout;
        config.base.max_retries = env.max_retries;
        if env.api_base.is_some() {
            config.base.api_base = env.api_base;
        }
        config
    }
}

impl ProviderConfig for StabilityConfig {
    fn validate(&self) -> Result<(), String> {
        self.base.validate(PROVIDER)
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

/// Native Stability AI provider.
#[derive(Debug, Clone)]
pub struct StabilityProvider {
    config: StabilityConfig,
    client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl StabilityProvider {
    pub fn new(config: StabilityConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|error| ProviderError::configuration(PROVIDER, error))?;
        let client = BaseHttpClient::new_for_provider_no_redirect(PROVIDER, config.base.clone())?;
        Ok(Self {
            config,
            client,
            models: supported_models(),
        })
    }

    pub fn from_env() -> Result<Self, ProviderError> {
        Self::new(StabilityConfig::from_env())
    }

    fn request_headers(&self, api_key: &str) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(self.config.base.headers.len() + 2);
        for (key, value) in &self.config.base.headers {
            if key.eq_ignore_ascii_case("authorization") || key.eq_ignore_ascii_case("accept") {
                continue;
            }
            headers.push(header_owned(key.clone(), value.clone()));
        }
        headers.push(header("Authorization", format!("Bearer {api_key}")));
        headers.push(header("Accept", "image/*".to_string()));
        headers
    }

    fn endpoint_for_model(&self, model: &str) -> Result<String, ProviderError> {
        let path = match model {
            "stable-image-core" => "v2beta/stable-image/generate/core",
            "stable-image-ultra" => "v2beta/stable-image/generate/ultra",
            "sd3" | "sd3-large" | "sd3-large-turbo" | "sd3-medium" | "sd3.5-large"
            | "sd3.5-large-turbo" | "sd3.5-medium" => "v2beta/stable-image/generate/sd3",
            _ => return Err(ProviderError::model_not_found(PROVIDER, model)),
        };
        Ok(format!(
            "{}/{}",
            self.config
                .base
                .api_base
                .as_deref()
                .unwrap_or(DEFAULT_API_BASE)
                .trim_end_matches('/'),
            path
        ))
    }

    fn form_for_request(
        &self,
        request: &ImageGenerationRequest,
        model: &str,
    ) -> Result<Form, ProviderError> {
        if request.n.is_some_and(|count| count != 1) {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "Stability native generation returns exactly one image",
            ));
        }
        if request.quality.is_some() || request.style.is_some() {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "Stability does not accept OpenAI quality or style parameters",
            ));
        }
        let output_format = match request.response_format.as_deref() {
            None | Some("b64_json") => "png",
            Some(format @ ("png" | "jpeg" | "webp")) => format,
            Some(other) => {
                return Err(ProviderError::invalid_request(
                    PROVIDER,
                    format!("unsupported Stability output format '{other}'"),
                ));
            }
        };
        let mut form = Form::new()
            .text("prompt", request.prompt.clone())
            .text("output_format", output_format.to_string());
        if let Some(size) = request.size.as_deref() {
            if size != "1024x1024"
                || !(model == "stable-image-core"
                    || model == "stable-image-ultra"
                    || model.starts_with("sd3"))
            {
                return Err(ProviderError::invalid_request(
                    PROVIDER,
                    format!("Stability model '{model}' cannot guarantee exact image size '{size}'"),
                ));
            }
            form = form.text("aspect_ratio", "1:1".to_string());
        }
        if model.starts_with("sd3") {
            let upstream_model = match model {
                "sd3" => "sd3.5-large",
                other => other,
            };
            form = form.text("model", upstream_model.to_string());
        }
        Ok(form)
    }

    async fn execute_generation(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let model = request.model.as_deref().unwrap_or("stable-image-core");
        let url = self.endpoint_for_model(model)?;
        let form = self.form_for_request(request, model)?;
        let api_key = self
            .config
            .base
            .api_key
            .as_deref()
            .ok_or_else(|| ProviderError::authentication(PROVIDER, "API key is required"))?;
        let response =
            apply_provider_headers(self.client.post(url)?, self.request_headers(api_key))
                .multipart(form)
                .send()
                .await
                .map_err(|error| self.client.map_preserved_request_error(error))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(error) if status.is_success() => return Err(post_header_body_error(error)),
            Err(error) => return Err(self.client.map_preserved_request_error(error)),
        };
        if !status.is_success() {
            return Err(map_error_response(status.as_u16(), &bytes));
        }
        ensure_image_body(&bytes, content_type.as_deref())?;
        Ok(ImageGenerationResponse {
            created: chrono::Utc::now().timestamp().unsigned_abs(),
            data: vec![ImageData {
                url: None,
                b64_json: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
                revised_prompt: None,
            }],
        })
    }

    async fn execute_edit(
        &self,
        request: ImageEditRequest,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        if request.n.is_some_and(|count| count != 1) {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "Stability native editing returns exactly one image",
            ));
        }
        if request.size.is_some() {
            return Err(ProviderError::invalid_request(
                PROVIDER,
                "Stability native editing does not support the OpenAI size parameter",
            ));
        }
        let model = request.model.as_deref().unwrap_or("inpaint");
        if model != "inpaint" {
            return Err(ProviderError::model_not_found(PROVIDER, model));
        }
        let output_format = match request.response_format.as_deref() {
            None | Some("b64_json" | "png") => "png",
            Some(format @ ("jpeg" | "webp")) => format,
            Some(other) => {
                return Err(ProviderError::invalid_request(
                    PROVIDER,
                    format!("unsupported Stability output format '{other}'"),
                ));
            }
        };
        let mut form = Form::new()
            .part("image", Part::bytes(request.image).file_name("image.png"))
            .text("prompt", request.prompt)
            .text("output_format", output_format.to_string());
        if let Some(mask) = request.mask {
            form = form.part("mask", Part::bytes(mask).file_name("mask.png"));
        }
        let api_key = self
            .config
            .base
            .api_key
            .as_deref()
            .ok_or_else(|| ProviderError::authentication(PROVIDER, "API key is required"))?;
        let url = format!(
            "{}/v2beta/stable-image/edit/inpaint",
            self.config
                .base
                .api_base
                .as_deref()
                .unwrap_or(DEFAULT_API_BASE)
                .trim_end_matches('/')
        );
        let response =
            apply_provider_headers(self.client.post(url)?, self.request_headers(api_key))
                .multipart(form)
                .send()
                .await
                .map_err(|error| self.client.map_preserved_request_error(error))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(error) if status.is_success() => return Err(post_header_body_error(error)),
            Err(error) => return Err(self.client.map_preserved_request_error(error)),
        };
        if !status.is_success() {
            return Err(map_error_response(status.as_u16(), &bytes));
        }
        ensure_image_body(&bytes, content_type.as_deref())?;
        Ok(ImageGenerationResponse {
            created: chrono::Utc::now().timestamp().unsigned_abs(),
            data: vec![ImageData {
                url: None,
                b64_json: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
                revised_prompt: None,
            }],
        })
    }
}

fn post_header_body_error(error: reqwest::Error) -> ProviderError {
    ProviderError::other(
        PROVIDER,
        format!("response body failed after Stability accepted the request: {error}"),
    )
}

fn ensure_image_body(body: &[u8], content_type: Option<&str>) -> Result<(), ProviderError> {
    if body.is_empty() {
        return Err(ProviderError::response_parsing(
            PROVIDER,
            "Stability response contained an empty image body",
        ));
    }
    let image_content_type = content_type
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().starts_with("image/"));
    let image_signature = body.starts_with(b"\x89PNG\r\n\x1a\n")
        || body.starts_with(b"\xff\xd8\xff")
        || (body.len() >= 12 && body.starts_with(b"RIFF") && &body[8..12] == b"WEBP");
    if !image_content_type && !image_signature {
        return Err(ProviderError::response_parsing(
            PROVIDER,
            "Stability response did not contain image content",
        ));
    }
    Ok(())
}

pub(crate) fn is_post_submit_error(error: &ProviderError) -> bool {
    matches!(error,
        ProviderError::Other { provider: PROVIDER, message }
            if message.starts_with("response body failed after Stability accepted the request:"))
        || matches!(
            error,
            ProviderError::ResponseParsing {
                provider: PROVIDER,
                ..
            }
        )
}

fn map_error_response(status: u16, body: &[u8]) -> ProviderError {
    let is_moderation = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|body| {
            body.get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .is_some_and(|name| name == "content_moderation");
    if status == 403 && is_moderation {
        ProviderError::content_filtered(
            PROVIDER,
            "request was flagged by Stability content moderation",
            None,
            Some(false),
        )
    } else {
        HttpErrorMapper::map_status_code(PROVIDER, status, &String::from_utf8_lossy(body))
    }
}

impl LLMProvider for StabilityProvider {
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
        self.execute_generation(&request).await
    }

    async fn image_edit(
        &self,
        request: ImageEditRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.execute_edit(request).await
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
            "token pricing is unavailable for per-image Stability models",
        ))
    }
}

fn supported_models() -> Vec<ModelInfo> {
    GENERATION_MODELS
        .iter()
        .map(|model| model_info(model, GENERATION_CAPABILITIES))
        .chain(std::iter::once(model_info("inpaint", EDIT_CAPABILITIES)))
        .collect()
}

fn model_info(model: &str, capabilities: &[ProviderCapability]) -> ModelInfo {
    ModelInfo {
        id: model.to_string(),
        name: model.to_string(),
        provider: PROVIDER.to_string(),
        max_context_length: 0,
        supports_multimodal: true,
        capabilities: capabilities.to_vec(),
        ..ModelInfo::default()
    }
}
