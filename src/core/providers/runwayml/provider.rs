//! Runway ML Provider Implementation
//!
//! Main provider implementation for Runway ML video and image generation.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, get_pricing_db, header,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
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

use super::{RunwayMLConfig, RunwayMLErrorMapper, get_runwayml_registry};

const PROVIDER_NAME: &str = "runwayml";

/// Runway ML task status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskStatus {
    /// Task is pending
    Pending,
    /// Task is in the queue
    Throttled,
    /// Task is running
    Running,
    /// Task completed successfully
    Succeeded,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Runway ML task request
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    /// The model to use (e.g., "gen3a_turbo")
    pub model: String,
    /// Text prompt for generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_text: Option<String>,
    /// Image URL for image-to-video
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_image: Option<String>,
    /// Video duration in seconds (5 or 10)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// Aspect ratio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<String>,
    /// Seed for reproducibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Whether to watermark the output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark: Option<bool>,
}

/// Runway ML task response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    /// Task ID
    pub id: String,
    /// Task status
    pub status: TaskStatus,
    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<String>,
    /// Output URLs (available when succeeded)
    #[serde(default)]
    pub output: Option<Vec<String>>,
    /// Error message (if failed)
    #[serde(default)]
    pub failure: Option<String>,
    /// Failure code
    #[serde(default)]
    pub failure_code: Option<String>,
    /// Progress percentage
    #[serde(default)]
    pub progress: Option<f32>,
}

/// Runway ML video generation response
#[derive(Debug, Clone)]
pub struct VideoGenerationResponse {
    /// Task ID
    pub task_id: String,
    /// Video URLs
    pub video_urls: Vec<String>,
    /// Generation duration
    pub duration_seconds: u32,
}

/// Runway ML provider implementation
#[derive(Debug, Clone)]
pub struct RunwayMLProvider {
    config: RunwayMLConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl RunwayMLProvider {
    /// Create a new Runway ML provider
    pub fn new(config: RunwayMLConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e.to_string()))?,
        );

        let supported_models = get_runwayml_registry().models().to_vec();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    /// Create provider with API key
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let config = RunwayMLConfig::new(api_key);
        Self::new(config)
    }

    /// Create provider from environment
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = RunwayMLConfig::from_env();
        Self::new(config)
    }

    /// Generate headers for Runway ML API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(3);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        headers.push(header("Content-Type", "application/json".to_string()));
        headers.push(header("Accept", "application/json".to_string()));

        // Add API version header if specified
        if let Some(api_version) = &self.config.base.api_version {
            headers.push(header("X-Runway-Version", api_version.clone()));
        }

        headers
    }

    /// Create a video generation task
    pub async fn create_video_task(
        &self,
        prompt_text: Option<String>,
        prompt_image: Option<String>,
        model: Option<&str>,
        duration: Option<u32>,
        ratio: Option<String>,
        seed: Option<u64>,
    ) -> Result<TaskResponse, ProviderError> {
        let api_model = model
            .map(|m| get_runwayml_registry().get_api_model(m))
            .unwrap_or("gen3a_turbo");

        let request = CreateTaskRequest {
            model: api_model.to_string(),
            prompt_text,
            prompt_image,
            duration: duration.or(Some(self.config.default_video_duration)),
            ratio,
            seed,
            watermark: Some(self.config.watermark),
        };

        self.submit_task(&request).await
    }

    /// Submit a task to Runway ML
    async fn submit_task(
        &self,
        request: &CreateTaskRequest,
    ) -> Result<TaskResponse, ProviderError> {
        let url = self.config.get_generate_url();
        let headers = self.get_request_headers();
        let body = serde_json::to_value(request)
            .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))?;

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            let mapper = RunwayMLErrorMapper;
            return Err(mapper.map_http_error(status.as_u16(), &error_text));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    /// Get task status
    async fn get_task(&self, task_id: &str) -> Result<TaskResponse, ProviderError> {
        let url = self.config.get_task_url(task_id);
        let headers = self.get_request_headers();

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None)
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            let mapper = RunwayMLErrorMapper;
            return Err(mapper.map_http_error(status.as_u16(), &error_text));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    /// Poll task until completion
    async fn poll_task(&self, task_id: &str) -> Result<TaskResponse, ProviderError> {
        let polling_delay = std::time::Duration::from_secs(self.config.polling_delay_seconds);

        for _ in 0..self.config.polling_retries {
            tokio::time::sleep(polling_delay).await;

            let task = self.get_task(task_id).await?;

            match task.status {
                TaskStatus::Succeeded => return Ok(task),
                TaskStatus::Failed => {
                    let error_msg = task.failure.unwrap_or_else(|| "Task failed".to_string());
                    return Err(ProviderError::api_error(
                        PROVIDER_NAME,
                        500,
                        format!("Video generation failed: {}", error_msg),
                    ));
                }
                TaskStatus::Cancelled => {
                    return Err(ProviderError::cancelled(
                        PROVIDER_NAME,
                        "video_generation",
                        Some("Task was cancelled".to_string()),
                    ));
                }
                _ => {
                    // Still processing, continue polling
                }
            }
        }

        Err(ProviderError::timeout(
            PROVIDER_NAME,
            "Maximum retries exceeded waiting for video generation",
        ))
    }

    /// Create video task and wait for completion
    pub async fn generate_video(
        &self,
        prompt_text: Option<String>,
        prompt_image: Option<String>,
        model: Option<&str>,
        duration: Option<u32>,
        ratio: Option<String>,
        seed: Option<u64>,
    ) -> Result<VideoGenerationResponse, ProviderError> {
        // Create the task
        let task = self
            .create_video_task(prompt_text, prompt_image, model, duration, ratio, seed)
            .await?;

        // Poll until completion
        let completed_task = self.poll_task(&task.id).await?;

        // Extract video URLs
        let video_urls = completed_task.output.unwrap_or_default();

        Ok(VideoGenerationResponse {
            task_id: completed_task.id,
            video_urls,
            duration_seconds: duration.unwrap_or(self.config.default_video_duration),
        })
    }

    /// Transform image generation request to video generation
    fn transform_image_to_video_request(
        &self,
        request: &ImageGenerationRequest,
    ) -> CreateTaskRequest {
        let registry = get_runwayml_registry();
        let model = request.model.as_deref().unwrap_or("gen3a_turbo");
        let api_model = registry.get_api_model(model);

        // Map size to aspect ratio
        let ratio = request.size.as_ref().map(|size| {
            match size.as_str() {
                "1024x1024" | "512x512" => "1:1",
                "1792x1024" | "1280x720" => "16:9",
                "1024x1792" | "720x1280" => "9:16",
                "1280x768" => "5:3",
                "768x1280" => "3:5",
                _ => "16:9", // Default to 16:9
            }
            .to_string()
        });

        CreateTaskRequest {
            model: api_model.to_string(),
            prompt_text: Some(request.prompt.clone()),
            prompt_image: None,
            duration: Some(self.config.default_video_duration),
            ratio,
            seed: None,
            watermark: Some(self.config.watermark),
        }
    }

    /// Transform video response to image generation response
    fn transform_video_to_image_response(
        &self,
        video_response: VideoGenerationResponse,
    ) -> ImageGenerationResponse {
        let data: Vec<ImageData> = video_response
            .video_urls
            .into_iter()
            .map(|url| ImageData {
                url: Some(url),
                b64_json: None,
                revised_prompt: None,
            })
            .collect();

        ImageGenerationResponse {
            created: chrono::Utc::now().timestamp() as u64,
            data,
        }
    }
}

#[async_trait]
impl LLMProvider for RunwayMLProvider {
    type Config = RunwayMLConfig;
    type Error = ProviderError;
    type ErrorMapper = RunwayMLErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[ProviderCapability::ImageGeneration]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["model", "prompt", "size", "n"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        let mut mapped = HashMap::new();

        for (key, value) in params {
            match key.as_str() {
                "size" => {
                    // Map OpenAI size to Runway ratio
                    if let Some(size_str) = value.as_str() {
                        let ratio = match size_str {
                            "1024x1024" | "512x512" => "1:1",
                            "1792x1024" | "1280x720" => "16:9",
                            "1024x1792" | "720x1280" => "9:16",
                            _ => "16:9",
                        };
                        mapped.insert("ratio".to_string(), Value::String(ratio.to_string()));
                    }
                }
                "n" => {
                    // Runway generates one video at a time
                    // Store for reference but don't pass to API
                    mapped.insert("_n".to_string(), value);
                }
                _ => {
                    mapped.insert(key, value);
                }
            }
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        // Runway ML is primarily for video/image generation, not chat
        Err(ProviderError::not_supported(
            PROVIDER_NAME,
            "Chat completion is not supported by Runway ML. Use image_generation for video generation.",
        ))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_supported(
            PROVIDER_NAME,
            "Chat completion is not supported by Runway ML",
        ))
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        RunwayMLErrorMapper
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        Err(ProviderError::not_supported(
            PROVIDER_NAME,
            "Chat completion is not supported by Runway ML. Use image_generation for video generation.",
        ))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        Err(ProviderError::not_supported(
            PROVIDER_NAME,
            "Streaming is not supported by Runway ML",
        ))
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        let task_request = self.transform_image_to_video_request(&request);

        // Submit the task
        let task = self.submit_task(&task_request).await?;

        // Poll until completion
        let completed_task = self.poll_task(&task.id).await?;

        // Build response
        let video_urls = completed_task.output.unwrap_or_default();
        let video_response = VideoGenerationResponse {
            task_id: completed_task.id,
            video_urls,
            duration_seconds: self.config.default_video_duration,
        };

        Ok(self.transform_video_to_image_response(video_response))
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.base.api_key.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // Runway pricing is per-second of video, not per token
        // Use pricing database for estimation if available
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(get_pricing_db().calculate(model, &usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation_without_api_key() {
        let config = RunwayMLConfig::default();
        let result = RunwayMLProvider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_creation_with_api_key() {
        let config = RunwayMLConfig::new("test-api-key");
        let result = RunwayMLProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();
        assert_eq!(provider.name(), PROVIDER_NAME);
    }

    #[test]
    fn test_provider_capabilities() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();
        let capabilities = provider.capabilities();
        assert!(capabilities.contains(&ProviderCapability::ImageGeneration));
    }

    #[test]
    fn test_provider_models() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();
        let models = provider.models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_get_request_headers() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();
        let headers = provider.get_request_headers();

        assert!(headers.iter().any(|h| h.0 == "Authorization"));
        assert!(headers.iter().any(|h| h.0 == "Content-Type"));
    }

    #[test]
    fn test_transform_image_to_video_request() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();

        let request = ImageGenerationRequest {
            prompt: "A beautiful sunset over the ocean".to_string(),
            model: Some("gen3a_turbo".to_string()),
            n: Some(1),
            size: Some("1792x1024".to_string()),
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };

        let task_request = provider.transform_image_to_video_request(&request);

        assert_eq!(task_request.model, "gen3a_turbo");
        assert_eq!(
            task_request.prompt_text,
            Some("A beautiful sunset over the ocean".to_string())
        );
        assert_eq!(task_request.ratio, Some("16:9".to_string()));
        assert_eq!(task_request.duration, Some(5));
    }

    #[test]
    fn test_transform_video_to_image_response() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();

        let video_response = VideoGenerationResponse {
            task_id: "task-123".to_string(),
            video_urls: vec!["https://example.com/video.mp4".to_string()],
            duration_seconds: 5,
        };

        let response = provider.transform_video_to_image_response(video_response);

        assert_eq!(response.data.len(), 1);
        assert!(response.data[0].url.is_some());
    }

    #[test]
    fn test_supported_openai_params() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();
        let params = provider.get_supported_openai_params("gen3a_turbo");

        assert!(params.contains(&"prompt"));
        assert!(params.contains(&"size"));
    }

    #[tokio::test]
    async fn test_chat_completion_not_supported() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();

        let request = ChatRequest {
            model: "gen3a_turbo".to_string(),
            messages: vec![],
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.chat_completion(request, context).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::NotSupported { .. }
        ));
    }

    #[test]
    fn test_health_check_with_api_key() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let health = rt.block_on(provider.health_check());
        assert_eq!(health, HealthStatus::Healthy);
    }

    #[test]
    fn test_from_env_missing_api_key() {
        // Clear any existing env var
        unsafe {
            std::env::remove_var("RUNWAYML_API_KEY");
        }

        let result = RunwayMLProvider::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn test_create_task_request_serialization() {
        let request = CreateTaskRequest {
            model: "gen3a_turbo".to_string(),
            prompt_text: Some("A cat playing piano".to_string()),
            prompt_image: None,
            duration: Some(5),
            ratio: Some("16:9".to_string()),
            seed: None,
            watermark: Some(false),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "gen3a_turbo");
        assert_eq!(json["promptText"], "A cat playing piano");
        assert_eq!(json["duration"], 5);
        assert_eq!(json["ratio"], "16:9");
    }

    #[test]
    fn test_task_status_deserialization() {
        let json =
            r#"{"id":"task-123","status":"SUCCEEDED","output":["https://example.com/video.mp4"]}"#;
        let task: TaskResponse = serde_json::from_str(json).unwrap();
        assert_eq!(task.status, TaskStatus::Succeeded);
        assert_eq!(task.output.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let config = RunwayMLConfig::new("test-api-key");
        let provider = RunwayMLProvider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("size".to_string(), serde_json::json!("1792x1024"));
        params.insert("n".to_string(), serde_json::json!(1));

        let mapped = provider
            .map_openai_params(params, "gen3a_turbo")
            .await
            .unwrap();

        assert!(mapped.contains_key("ratio"));
        assert_eq!(mapped.get("ratio").unwrap(), "16:9");
    }
}
