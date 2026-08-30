//! Native Stability AI image generation and editing transport.

use base64::Engine as _;
use futures::Stream;
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
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
        let client = BaseHttpClient::new_for_provider(PROVIDER, config.base.clone())?;
        Ok(Self {
            config,
            client,
            models: supported_models(),
        })
    }

    pub fn from_env() -> Result<Self, ProviderError> {
        Self::new(StabilityConfig::from_env())
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
            let aspect_ratio = match size {
                "1024x1024" | "512x512" => "1:1",
                "1792x1024" | "1280x720" => "16:9",
                "1024x1792" | "720x1280" => "9:16",
                _ => {
                    return Err(ProviderError::invalid_request(
                        PROVIDER,
                        format!("unsupported Stability image size '{size}'"),
                    ));
                }
            };
            form = form.text("aspect_ratio", aspect_ratio.to_string());
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
        let response = self
            .client
            .post(url)?
            .bearer_auth(api_key)
            .header(reqwest::header::ACCEPT, "image/*")
            .multipart(form)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| ProviderError::network(PROVIDER, error.to_string()))?;
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                PROVIDER,
                status.as_u16(),
                &String::from_utf8_lossy(&bytes),
            ));
        }
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
        let response = self
            .client
            .post(url)?
            .bearer_auth(api_key)
            .header(reqwest::header::ACCEPT, "image/*")
            .multipart(form)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| ProviderError::network(PROVIDER, error.to_string()))?;
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                PROVIDER,
                status.as_u16(),
                &String::from_utf8_lossy(&bytes),
            ));
        }
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
