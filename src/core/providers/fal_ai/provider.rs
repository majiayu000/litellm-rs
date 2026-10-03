//! Fal AI Provider Implementation
//!
//! Main provider implementation for Fal AI image generation

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpErrorMapper, apply_headers, header,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    provider::ProviderConfig, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    image::ImageGenerationRequest,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, ImageData, ImageGenerationResponse},
};

use super::models::map_openai_to_fal_params;
use super::{FalAIConfig, FalAIErrorMapper, FalAIModelRegistry};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// Fal AI Provider for image generation
#[derive(Debug, Clone)]
pub struct FalAIProvider {
    config: FalAIConfig,
    pool_manager: Arc<GlobalPoolManager>,
    model_registry: FalAIModelRegistry,
    supported_models: Vec<ModelInfo>,
}

impl FalAIProvider {
    /// Create a new Fal AI provider with configuration
    pub fn new(config: FalAIConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("fal_ai", e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("fal_ai", e.to_string()))?,
        );

        let model_registry = FalAIModelRegistry::new();
        let supported_models = Self::build_model_info(&model_registry);

        Ok(Self {
            config,
            pool_manager,
            model_registry,
            supported_models,
        })
    }

    /// Create provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = FalAIConfig::from_env();
        Self::new(config)
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = FalAIConfig::with_api_key(api_key);
        Self::new(config)
    }

    /// Build ModelInfo list from registry
    fn build_model_info(registry: &FalAIModelRegistry) -> Vec<ModelInfo> {
        registry
            .list_models()
            .iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                provider: "fal_ai".to_string(),
                max_context_length: 0,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: None,
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::ImageGeneration],
                created_at: None,
                updated_at: None,
                metadata: std::collections::HashMap::new(),
            })
            .collect()
    }

    /// Generate request headers for Fal AI API
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = self.config.get_api_key() {
            headers.push(header("Authorization", format!("Key {}", api_key)));
        }

        headers.push(header("Content-Type", "application/json".to_string()));
        headers
    }

    /// Get the endpoint URL for a model
    fn get_model_endpoint(&self, model: &str) -> String {
        let base = self.config.get_api_base().trim_end_matches('/');
        format!("{}/{}", base, model)
    }

    /// Transform Fal AI response to ImageGenerationResponse
    fn transform_image_response(
        &self,
        response_data: Value,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let images = response_data
            .get("images")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                ProviderError::response_parsing("fal_ai", "Missing 'images' field in response")
            })?;

        let data: Vec<ImageData> = images
            .iter()
            .filter_map(|img| {
                if let Some(url) = img.get("url").and_then(|v| v.as_str()) {
                    Some(ImageData {
                        url: Some(url.to_string()),
                        b64_json: img
                            .get("b64_json")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        revised_prompt: None,
                    })
                } else {
                    img.as_str().map(|url| ImageData {
                        url: Some(url.to_string()),
                        b64_json: None,
                        revised_prompt: None,
                    })
                }
            })
            .collect();

        let created = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(ImageGenerationResponse { created, data })
    }

    fn build_image_request_body(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<Value, ProviderError> {
        let model_id = request.model.as_deref().unwrap_or("fal-ai/flux/schnell");
        let model = self
            .model_registry
            .get(model_id)
            .ok_or_else(|| ProviderError::model_not_found("fal_ai", model_id))?;
        let n = request.n.unwrap_or(1);
        if n == 0 || n > model.max_images {
            return Err(ProviderError::invalid_request(
                "fal_ai",
                format!("{model_id} supports 1..={} images", model.max_images),
            ));
        }
        let mut body = serde_json::json!({"prompt": request.prompt});
        if model.max_images > 1 {
            // BRIA defaults to four upstream; OpenAI-compatible omission means one.
            body["num_images"] = serde_json::json!(n);
        }
        if let Some(size) = &request.size {
            if model.supported_sizes.is_empty() {
                return Err(ProviderError::not_supported(
                    "fal_ai",
                    "This endpoint accepts aspect_ratio, not an exact pixel size",
                ));
            }
            let (width, height) = size
                .split_once('x')
                .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
                .filter(|(w, h)| *w > 0 && *h > 0)
                .ok_or_else(|| {
                    ProviderError::invalid_request("fal_ai", "size must be positive WIDTHxHEIGHT")
                })?;
            body["image_size"] = serde_json::json!({"width": width, "height": height});
        }
        // Recraft's text-to-image schemas expose neither of these transport options.
        let recraft = matches!(
            model_id,
            "fal-ai/recraft/v3/text-to-image"
                | "fal-ai/recraft/v4/text-to-image"
                | "fal-ai/recraft/v4/pro/text-to-image"
        );
        if !recraft {
            body["sync_mode"] = serde_json::json!(self.config.sync_mode);
        }
        if let Some(format) = request.response_format.as_deref()
            && format != "url"
        {
            return Err(ProviderError::not_supported(
                "fal_ai",
                "Only response_format=url is supported",
            ));
        }
        if !recraft
            && !matches!(
                model_id,
                "fal-ai/bria/text-to-image/hd"
                    | "fal-ai/stable-diffusion-v3-medium"
                    | "fal-ai/ideogram/v3"
            )
        {
            body["output_format"] = serde_json::json!(self.config.output_format);
        }
        Ok(body)
    }

    fn transport_attempts(&self) -> u32 {
        self.config.base.max_retries.saturating_add(1)
    }

    async fn execute_image_request(
        &self,
        url: &str,
        headers: Vec<HeaderPair>,
        body: Value,
    ) -> Result<reqwest::Response, ProviderError> {
        let mut last_error = None;

        for attempt in 0..self.transport_attempts() {
            let request_builder = self
                .pool_manager
                .client()
                .post(url)
                .timeout(self.config.timeout())
                .json(&body);
            let request_builder = apply_headers(request_builder, headers.clone());

            match request_builder.send().await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    let provider_err = if err.is_timeout() {
                        ProviderError::timeout("fal_ai", err.to_string())
                    } else {
                        ProviderError::network("fal_ai", err.to_string())
                    };
                    if attempt + 1 == self.transport_attempts() {
                        return Err(provider_err);
                    }
                    last_error = Some(provider_err);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| ProviderError::network("fal_ai", "request failed before execution")))
    }
}

impl LLMProvider for FalAIProvider {
    fn name(&self) -> &'static str {
        "fal_ai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[ProviderCapability::ImageGeneration]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        match self.model_registry.get(model) {
            None => &[],
            Some(model) if model.supported_sizes.is_empty() => &["n", "response_format"],
            Some(_) => super::models::SUPPORTED_OPENAI_PARAMS,
        }
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        let params_value = serde_json::to_value(&params)
            .map_err(|e| ProviderError::invalid_request("fal_ai", e.to_string()))?;
        let mapped = map_openai_to_fal_params(&params_value);

        serde_json::from_value(mapped)
            .map_err(|e| ProviderError::invalid_request("fal_ai", e.to_string()))
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        // Fal AI is primarily for image generation, not chat
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Chat completion not supported. Use image_generation instead.",
        ))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Chat response transformation not supported",
        ))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(FalAIErrorMapper)
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Chat completion not supported. Fal AI is an image generation provider.",
        ))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        Err(ProviderError::not_implemented(
            "fal_ai",
            "Streaming not supported for image generation",
        ))
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        let model = request.model.as_deref().unwrap_or("fal-ai/flux/schnell");
        let url = self.get_model_endpoint(model);
        let body = self.build_image_request_body(&request)?;

        let headers = self.get_request_headers();

        let response = self.execute_image_request(&url, headers, body).await?;

        let status = response.status();
        let response_bytes = match response.bytes().await {
            Ok(response_bytes) => response_bytes,
            Err(_) if !status.is_success() => {
                return Err(HttpErrorMapper::map_status_code(
                    "fal_ai",
                    status.as_u16(),
                    "failed to read upstream error body",
                ));
            }
            Err(error) => return Err(ProviderError::network("fal_ai", error.to_string())),
        };

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            return Err(HttpErrorMapper::map_status_code(
                "fal_ai",
                status.as_u16(),
                &error_text,
            ));
        }

        let response_data: Value = serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing("fal_ai", e.to_string()))?;

        self.transform_image_response(response_data)
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.get_api_key().is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        if !self.model_registry.is_supported(model) {
            return Err(ProviderError::model_not_found("fal_ai", model));
        }
        // A unit image price is available only when the official tariff is fixed.
        self.model_registry.get_cost_per_image(model).ok_or_else(||
            ProviderError::not_supported("fal_ai", "This model requires image dimensions, rendering mode, style, or generation count for pricing; token counts cannot determine its cost"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn forbidden_fal_api_base() -> std::io::Result<String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        tokio::spawn(async move {
            let (mut socket, _) = listener
                .accept()
                .await
                .expect("Fal AI test server should accept request");
            let mut request = [0_u8; 4096];
            let read = socket
                .read(&mut request)
                .await
                .expect("Fal AI test server should read request");
            assert!(read > 0, "Fal AI test server should receive a request");
            let body = r#"{"detail":"model access denied"}"#;
            let response = format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("Fal AI test server should write response");
        });
        Ok(format!("http://{address}"))
    }

    #[tokio::test]
    async fn current_image_models_do_not_claim_flat_megapixel_costs() {
        let provider = FalAIProvider::new(FalAIConfig::with_api_key("test-key")).unwrap();
        for model in [
            "fal-ai/flux-2-pro",
            "fal-ai/flux-2-flex",
            "ideogram/v4",
            "fal-ai/flux/schnell",
            "fal-ai/flux-pro/v1.1",
            "fal-ai/flux-pro/v1.1-ultra",
            "fal-ai/ideogram/v3",
            "fal-ai/recraft/v3/text-to-image",
            "fal-ai/bria/text-to-image/hd",
        ] {
            assert!(provider.models().iter().any(|m| m.id == model));
            assert!(provider.calculate_cost(model, 0, 0).await.is_err());
        }
        assert_eq!(
            provider
                .calculate_cost("fal-ai/recraft/v4/text-to-image", 0, 0)
                .await
                .unwrap(),
            0.04
        );
    }

    fn image_request(model: &str) -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "a bird".into(),
            model: Some(model.into()),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        }
    }

    #[tokio::test]
    async fn unavailable_models_do_not_route_or_price_as_free() {
        let provider = FalAIProvider::new(FalAIConfig::with_api_key("test-key")).unwrap();
        let wrapped = crate::core::providers::Provider::FalAI(provider.clone());
        for id in ["fal-ai/imagen4/preview", "unknown-model"] {
            assert!(
                !wrapped.supports_capability_for_model(id, &ProviderCapability::ImageGeneration)
            );
            assert!(matches!(
                provider.calculate_cost(id, 0, 0).await,
                Err(ProviderError::ModelNotFound { .. })
            ));
            assert!(matches!(
                provider
                    .image_generation(image_request(id), RequestContext::default())
                    .await,
                Err(ProviderError::ModelNotFound { .. })
            ));
        }
        assert!(wrapped.supports_capability_for_model(
            "fal-ai/flux/schnell",
            &ProviderCapability::ImageGeneration
        ));
        assert!(!wrapped.supports_capability_for_model(
            "fal-ai/flux/schnell",
            &ProviderCapability::ChatCompletion
        ));
    }

    #[test]
    fn model_specific_image_arguments_fail_explicitly() {
        let provider = FalAIProvider::new(FalAIConfig::with_api_key("test-key")).unwrap();
        for id in [
            "fal-ai/recraft/v3/text-to-image",
            "fal-ai/recraft/v4/text-to-image",
            "fal-ai/recraft/v4/pro/text-to-image",
            "fal-ai/flux-2-pro",
            "fal-ai/flux-2-flex",
        ] {
            let mut request = image_request(id);
            request.n = Some(2);
            assert!(provider.build_image_request_body(&request).is_err());
        }
        for id in ["fal-ai/flux-pro/v1.1-ultra", "fal-ai/bria/text-to-image/hd"] {
            let mut request = image_request(id);
            request.size = Some("1024x1024".into());
            assert!(provider.build_image_request_body(&request).is_err());
        }
        let mut request = image_request("fal-ai/flux/schnell");
        for n in [0, 5] {
            request.n = Some(n);
            assert!(provider.build_image_request_body(&request).is_err());
        }
        request.n = None;
        for size in ["invalid", "0x1024"] {
            request.size = Some(size.into());
            assert!(provider.build_image_request_body(&request).is_err());
        }
    }

    #[tokio::test]
    async fn http_image_requests_follow_endpoint_schemas() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut bodies = Vec::new();
            for _ in 0..4 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0; 4096];
                let (header_end, length) = loop {
                    let read = socket.read(&mut chunk).await.unwrap();
                    assert!(read > 0);
                    bytes.extend_from_slice(&chunk[..read]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length: usize = headers
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap())
                            })
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < header_end + length {
                    let read = socket.read(&mut chunk).await.unwrap();
                    assert!(read > 0);
                    bytes.extend_from_slice(&chunk[..read]);
                }
                bodies.push(
                    serde_json::from_slice::<Value>(&bytes[header_end..header_end + length])
                        .unwrap(),
                );
                let body = r#"{"images":[{"url":"https://example.com/image.webp"}]}"#;
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
            bodies
        });
        let mut config = FalAIConfig::with_api_key("test-key");
        config.base.api_base = Some(format!("http://{address}"));
        let provider = FalAIProvider::new(config).unwrap();
        for id in [
            "fal-ai/recraft/v4/text-to-image",
            "fal-ai/bria/text-to-image/hd",
            "fal-ai/flux/schnell",
            "fal-ai/ideogram/v3",
        ] {
            let mut request = image_request(id);
            if id == "fal-ai/flux/schnell" {
                request.size = Some("1792x1024".into());
            }
            if id == "fal-ai/ideogram/v3" {
                request.n = Some(8);
            }
            let response = provider
                .image_generation(request, RequestContext::default())
                .await
                .unwrap();
            assert_eq!(response.data.len(), 1);
        }
        let bodies = server.await.unwrap();
        assert_eq!(bodies[0], serde_json::json!({"prompt":"a bird"}));
        assert_eq!(bodies[1]["num_images"], 1);
        assert!(bodies[1].get("output_format").is_none());
        assert_eq!(
            bodies[2]["image_size"],
            serde_json::json!({"width":1792,"height":1024})
        );
        assert_eq!(bodies[3]["num_images"], 8);
    }

    #[test]
    fn test_provider_creation_fails_without_api_key() {
        let config = FalAIConfig::default();
        let result = FalAIProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_creation_with_api_key() {
        let config = FalAIConfig::with_api_key("test-key");
        let result = FalAIProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        assert_eq!(provider.name(), "fal_ai");
    }

    #[test]
    fn test_provider_capabilities() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ImageGeneration));
    }

    #[test]
    fn test_provider_models() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let models = provider.models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_get_model_endpoint() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let endpoint = provider.get_model_endpoint("fal-ai/flux/schnell");
        assert!(endpoint.contains("fal.run"));
        assert!(endpoint.contains("fal-ai/flux/schnell"));
    }

    #[test]
    fn test_transform_image_response() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let response_data = serde_json::json!({
            "images": [
                {"url": "https://example.com/image1.png"},
                {"url": "https://example.com/image2.png"}
            ]
        });

        let result = provider.transform_image_response(response_data);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.data.len(), 2);
        assert!(response.data[0].url.is_some());
    }

    #[test]
    fn test_transform_image_response_url_strings() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let response_data = serde_json::json!({
            "images": [
                "https://example.com/image1.png",
                "https://example.com/image2.png"
            ]
        });

        let result = provider.transform_image_response(response_data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform_image_response_missing_images() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let response_data = serde_json::json!({
            "error": "Something went wrong"
        });

        let result = provider.transform_image_response(response_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_supported_openai_params() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();
        let params = provider.get_supported_openai_params("fal-ai/flux/schnell");
        assert!(params.contains(&"n"));
        assert!(params.contains(&"size"));
        assert!(params.contains(&"response_format"));
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let cost = provider
            .calculate_cost("fal-ai/stable-diffusion-v3-medium", 0, 0)
            .await;
        assert!(cost.is_ok());
        assert!(cost.unwrap() > 0.0);
    }

    #[tokio::test]
    async fn test_health_check_with_api_key() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let status = provider.health_check().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_chat_completion_not_supported() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "test".to_string(),
            messages: vec![],
            ..Default::default()
        };
        let context = RequestContext::default();

        let result = provider.chat_completion(request, context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn image_generation_preserves_upstream_403() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = FalAIConfig::with_api_key("test-key");
        config.base.api_base = Some(forbidden_fal_api_base().await?);
        config.base.max_retries = 0;
        let provider = FalAIProvider::new(config)?;
        let request = ImageGenerationRequest {
            prompt: "restricted model".to_string(),
            model: Some("fal-ai/flux/schnell".to_string()),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };

        let error = provider
            .image_generation(request, RequestContext::default())
            .await
            .expect_err("Fal AI 403 should be returned as a permission failure");

        assert!(matches!(
            error,
            ProviderError::ApiError {
                status: 403,
                ref message,
                ..
            } if message.contains("model access denied")
        ));
        Ok(())
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let config = FalAIConfig::with_api_key("test-key");
        let provider = FalAIProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("n".to_string(), serde_json::json!(2));
        params.insert("size".to_string(), serde_json::json!("1024x1024"));

        let result = provider.map_openai_params(params, "model").await;
        assert!(result.is_ok());

        let mapped = result.unwrap();
        assert!(mapped.contains_key("num_images"));
        assert!(mapped.contains_key("image_size"));
    }

    #[test]
    fn test_build_image_request_body_uses_config_defaults() {
        let mut config = FalAIConfig::with_api_key("test-key");
        config.output_format = "png".to_string();
        config.sync_mode = false;
        let provider = FalAIProvider::new(config).unwrap();

        let body = provider
            .build_image_request_body(&ImageGenerationRequest {
                prompt: "test prompt".to_string(),
                model: Some("fal-ai/flux/schnell".to_string()),
                n: Some(2),
                size: Some("1024x1024".to_string()),
                quality: None,
                response_format: None,
                style: None,
                user: None,
            })
            .unwrap();

        assert_eq!(body["prompt"], "test prompt");
        assert_eq!(body["num_images"], 2);
        assert_eq!(body["output_format"], "png");
        assert_eq!(body["sync_mode"], false);
        assert_eq!(
            body["image_size"],
            serde_json::json!({"width":1024,"height":1024})
        );
    }

    #[test]
    fn test_build_image_request_body_url_preserves_configured_encoding() {
        let mut config = FalAIConfig::with_api_key("test-key");
        config.output_format = "png".to_string();
        let provider = FalAIProvider::new(config).unwrap();

        let body = provider
            .build_image_request_body(&ImageGenerationRequest {
                prompt: "test prompt".to_string(),
                model: Some("fal-ai/flux/schnell".to_string()),
                n: None,
                size: None,
                quality: None,
                response_format: Some("url".to_string()),
                style: None,
                user: None,
            })
            .unwrap();

        assert_eq!(body["output_format"], "png");
    }

    #[test]
    fn test_transport_attempts_include_initial_request() {
        let mut config = FalAIConfig::with_api_key("test-key");
        config.base.max_retries = 2;
        let provider = FalAIProvider::new(config).unwrap();

        assert_eq!(provider.transport_attempts(), 3);
    }
}
