//! Replicate Client
//!
//! Request transformation, response processing, and API client logic

use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

use super::models::{ReplicateModelType, get_replicate_registry};
use super::prediction::{CreatePredictionRequest, PredictionResponse, PredictionStatus};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    image::ImageGenerationRequest,
    message::MessageContent,
    message::MessageRole,
    model::ModelInfo,
    responses::{ChatChoice, ChatResponse, ImageData, ImageGenerationResponse, Usage},
};

/// Replicate API client logic
pub struct ReplicateClient;

impl ReplicateClient {
    /// Transform a ChatRequest to Replicate prediction input format
    pub fn transform_chat_request(request: &ChatRequest) -> Value {
        // Build the prompt from messages
        let prompt = Self::build_prompt_from_messages(&request.messages);

        // Build input parameters
        let mut input = json!({
            "prompt": prompt
        });

        // Add optional parameters
        if let Some(temp) = request.temperature {
            input["temperature"] = json!(temp);
        }

        if let Some(max_tokens) = request.max_tokens {
            input["max_new_tokens"] = json!(max_tokens);
        } else if let Some(max_completion_tokens) = request.max_completion_tokens {
            input["max_new_tokens"] = json!(max_completion_tokens);
        }

        if let Some(top_p) = request.top_p {
            input["top_p"] = json!(top_p);
        }

        // Handle stop sequences
        if let Some(stop) = &request.stop {
            input["stop_sequences"] = json!(stop.join(","));
        }

        // Handle seed
        if let Some(seed) = request.seed {
            input["seed"] = json!(seed);
        }

        // Extract system prompt if present
        let system_prompt = request.messages.iter().find_map(|msg| {
            if msg.role == crate::core::types::message::MessageRole::System {
                msg.content.as_ref().map(|c| c.to_string())
            } else {
                None
            }
        });

        if let Some(system) = system_prompt {
            input["system_prompt"] = json!(system);
        }

        input
    }

    /// Build a text prompt from chat messages
    fn build_prompt_from_messages(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();

        for msg in messages {
            let role_prefix = match msg.role {
                MessageRole::System | MessageRole::Developer => "[INST] <<SYS>>\n",
                MessageRole::User => "[INST] ",
                MessageRole::Assistant => "",
                MessageRole::Tool => "[TOOL] ",
                MessageRole::Function => "[FUNCTION] ",
            };

            let role_suffix = match msg.role {
                MessageRole::System | MessageRole::Developer => "\n<</SYS>>\n\n",
                MessageRole::User => " [/INST] ",
                MessageRole::Assistant => " </s><s>",
                MessageRole::Tool => " ",
                MessageRole::Function => " ",
            };

            if let Some(content) = &msg.content {
                let text = content.to_string();
                prompt.push_str(role_prefix);
                prompt.push_str(&text);
                prompt.push_str(role_suffix);
            }
        }

        prompt
    }

    /// Transform a PredictionResponse to a ChatResponse
    pub fn transform_prediction_to_chat_response(
        prediction: &PredictionResponse,
        model: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Check prediction status
        if !prediction.is_success() {
            let error_msg = prediction
                .error
                .clone()
                .unwrap_or_else(|| "Prediction failed".to_string());
            return Err(ProviderError::replicate_prediction_failed(error_msg));
        }

        // Get the output text
        let content = prediction
            .get_text_output()
            .unwrap_or_else(|| " ".to_string());

        // Build response
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Ok(ChatResponse {
            id: prediction.id.clone(),
            object: "chat.completion".to_string(),
            created: timestamp,
            model: format!("replicate/{}", model),
            system_fingerprint: None,
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text(content.clone())),
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                },
                logprobs: None,
                finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
            }],
            usage: Some(Usage {
                prompt_tokens: 0, // Replicate doesn't provide token counts
                completion_tokens: 0,
                total_tokens: 0,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
        })
    }

    /// Transform an ImageGenerationRequest to Replicate prediction input format
    pub fn transform_image_request(request: &ImageGenerationRequest, model: &str) -> Value {
        let registry = get_replicate_registry();
        let default_params = registry.get_default_params(model);

        let mut input = json!({
            "prompt": request.prompt
        });

        // Parse size if provided
        if let Some(size) = &request.size {
            if let Some((w, h)) = size.split_once('x')
                && let (Ok(width), Ok(height)) = (w.parse::<i64>(), h.parse::<i64>())
            {
                input["width"] = json!(width);
                input["height"] = json!(height);
            }
        } else if let Some(params) = default_params {
            // Use default size from model registry
            if let Some(width) = params.get("width") {
                input["width"] = width.clone();
            }
            if let Some(height) = params.get("height") {
                input["height"] = height.clone();
            }
        }

        // Add number of outputs
        if let Some(n) = request.n {
            input["num_outputs"] = json!(n);
        }

        // Add quality/guidance scale
        if let Some(quality) = &request.quality {
            // Map quality to guidance_scale
            let guidance_scale = match quality.as_str() {
                "hd" => 8.0,
                "standard" => 7.5,
                _ => 7.5,
            };
            input["guidance_scale"] = json!(guidance_scale);
        }

        // Add style/scheduler for SDXL
        if let Some(style) = &request.style {
            match style.as_str() {
                "vivid" => {
                    input["scheduler"] = json!("K_EULER_ANCESTRAL");
                }
                "natural" => {
                    input["scheduler"] = json!("DDIM");
                }
                _ => {}
            }
        }

        input
    }

    /// Transform a PredictionResponse to an ImageGenerationResponse
    pub fn transform_prediction_to_image_response(
        prediction: &PredictionResponse,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        // Check prediction status
        if !prediction.is_success() {
            let error_msg = prediction
                .error
                .clone()
                .unwrap_or_else(|| "Image generation failed".to_string());
            return Err(ProviderError::replicate_prediction_failed(error_msg));
        }

        // Get the output URLs
        let urls = prediction
            .get_image_urls()
            .ok_or_else(|| ProviderError::replicate_response_parsing("No images in output"))?;

        // Build response
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let data: Vec<ImageData> = urls
            .into_iter()
            .map(|url| ImageData {
                url: Some(url),
                b64_json: None,
                revised_prompt: None,
            })
            .collect();

        Ok(ImageGenerationResponse {
            created: timestamp,
            data,
        })
    }

    /// Create a prediction request
    pub fn create_prediction_request(
        input: Value,
        version: Option<String>,
        stream: bool,
    ) -> CreatePredictionRequest {
        let mut request = CreatePredictionRequest::new(input);

        if let Some(v) = version {
            request = request.with_version(v);
        }

        if stream {
            request = request.with_stream(true);
        }

        request
    }

    /// Get supported models
    pub fn supported_models() -> Vec<ModelInfo> {
        get_replicate_registry().get_all_models()
    }

    /// Get supported OpenAI parameters for chat completions
    pub fn supported_openai_params() -> &'static [&'static str] {
        &[
            "temperature",
            "max_tokens",
            "top_p",
            "stop",
            "seed",
            "stream",
        ]
    }

    /// Get model type
    pub fn get_model_type(model: &str) -> ReplicateModelType {
        get_replicate_registry()
            .get_model_type(model)
            .unwrap_or(ReplicateModelType::TextGeneration)
    }

    /// Check if a prediction is complete
    pub fn is_prediction_complete(status: &PredictionStatus) -> bool {
        status.is_terminal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_test_chat_request() -> ChatRequest {
        ChatRequest {
            model: "meta/llama-2-70b-chat".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello, how are you?".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: false,
            stream_options: None,
            tools: None,
            tool_choice: None,
            user: None,
            response_format: None,
            seed: None,
            max_completion_tokens: None,
            stop: None,
            parallel_tool_calls: None,
            n: None,
            logit_bias: None,
            functions: None,
            function_call: None,
            logprobs: None,
            top_logprobs: None,
            reasoning_effort: None,
            store: None,
            metadata: None,
            service_tier: None,
            thinking: None,
            extra_params: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_transform_chat_request() {
        let request = create_test_chat_request();
        let input = ReplicateClient::transform_chat_request(&request);

        assert!(input.get("prompt").is_some());
        // Use approximate comparison for floating point
        let temp = input["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.01);
        assert_eq!(input["max_new_tokens"], 100);
    }

    #[test]
    fn test_transform_chat_request_with_system() {
        let mut request = create_test_chat_request();
        request.messages.insert(
            0,
            ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text(
                    "You are a helpful assistant.".to_string(),
                )),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            },
        );

        let input = ReplicateClient::transform_chat_request(&request);
        assert!(input.get("system_prompt").is_some());
        assert_eq!(input["system_prompt"], "You are a helpful assistant.");
    }

    #[test]
    fn test_transform_chat_request_with_stop() {
        let mut request = create_test_chat_request();
        request.stop = Some(vec!["END".to_string(), "STOP".to_string()]);

        let input = ReplicateClient::transform_chat_request(&request);
        assert_eq!(input["stop_sequences"], "END,STOP");
    }

    #[test]
    fn test_transform_prediction_to_chat_response() {
        let prediction = PredictionResponse {
            id: "test-id".to_string(),
            version: None,
            status: PredictionStatus::Succeeded,
            input: None,
            output: Some(serde_json::json!("Hello! I'm doing well, thank you.")),
            error: None,
            logs: None,
            metrics: None,
            urls: None,
            created_at: None,
            started_at: None,
            completed_at: None,
            model: None,
            data_removed: None,
        };

        let response =
            ReplicateClient::transform_prediction_to_chat_response(&prediction, "llama-2-70b-chat");
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.id, "test-id");
        assert_eq!(response.choices.len(), 1);
        if let Some(MessageContent::Text(text)) = &response.choices[0].message.content {
            assert!(text.contains("Hello"));
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_transform_prediction_failed() {
        let prediction = PredictionResponse {
            id: "test-id".to_string(),
            version: None,
            status: PredictionStatus::Failed,
            input: None,
            output: None,
            error: Some("Model error".to_string()),
            logs: None,
            metrics: None,
            urls: None,
            created_at: None,
            started_at: None,
            completed_at: None,
            model: None,
            data_removed: None,
        };

        let response =
            ReplicateClient::transform_prediction_to_chat_response(&prediction, "llama-2-70b-chat");
        assert!(response.is_err());
    }

    #[test]
    fn test_transform_image_request() {
        let request = ImageGenerationRequest {
            prompt: "A beautiful sunset over mountains".to_string(),
            model: Some("stability-ai/sdxl".to_string()),
            n: Some(2),
            size: Some("1024x1024".to_string()),
            quality: Some("hd".to_string()),
            response_format: None,
            style: Some("vivid".to_string()),
            user: None,
        };

        let input = ReplicateClient::transform_image_request(&request, "stability-ai/sdxl");

        assert_eq!(input["prompt"], "A beautiful sunset over mountains");
        assert_eq!(input["width"], 1024);
        assert_eq!(input["height"], 1024);
        assert_eq!(input["num_outputs"], 2);
        assert_eq!(input["guidance_scale"], 8.0); // hd quality
        assert_eq!(input["scheduler"], "K_EULER_ANCESTRAL"); // vivid style
    }

    #[test]
    fn test_transform_prediction_to_image_response() {
        let prediction = PredictionResponse {
            id: "test-id".to_string(),
            version: None,
            status: PredictionStatus::Succeeded,
            input: None,
            output: Some(serde_json::json!([
                "https://example.com/image1.png",
                "https://example.com/image2.png"
            ])),
            error: None,
            logs: None,
            metrics: None,
            urls: None,
            created_at: None,
            started_at: None,
            completed_at: None,
            model: None,
            data_removed: None,
        };

        let response = ReplicateClient::transform_prediction_to_image_response(&prediction);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(
            response.data[0].url,
            Some("https://example.com/image1.png".to_string())
        );
    }

    #[test]
    fn test_create_prediction_request() {
        let input = json!({"prompt": "test"});
        let request =
            ReplicateClient::create_prediction_request(input.clone(), Some("v1".to_string()), true);

        assert_eq!(request.input, input);
        assert_eq!(request.version, Some("v1".to_string()));
        assert_eq!(request.stream, Some(true));
    }

    #[test]
    fn test_supported_models() {
        let models = ReplicateClient::supported_models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_supported_openai_params() {
        let params = ReplicateClient::supported_openai_params();
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"stream"));
    }

    #[test]
    fn test_get_model_type() {
        assert_eq!(
            ReplicateClient::get_model_type("meta/llama-2-70b-chat"),
            ReplicateModelType::TextGeneration
        );
        assert_eq!(
            ReplicateClient::get_model_type("stability-ai/sdxl"),
            ReplicateModelType::ImageGeneration
        );
    }

    #[test]
    fn test_is_prediction_complete() {
        assert!(ReplicateClient::is_prediction_complete(
            &PredictionStatus::Succeeded
        ));
        assert!(ReplicateClient::is_prediction_complete(
            &PredictionStatus::Failed
        ));
        assert!(ReplicateClient::is_prediction_complete(
            &PredictionStatus::Canceled
        ));
        assert!(!ReplicateClient::is_prediction_complete(
            &PredictionStatus::Processing
        ));
        assert!(!ReplicateClient::is_prediction_complete(
            &PredictionStatus::Starting
        ));
    }

    #[test]
    fn test_build_prompt_from_messages() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("Hi there!".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            },
        ];

        let prompt = ReplicateClient::build_prompt_from_messages(&messages);
        assert!(prompt.contains("Hello"));
        assert!(prompt.contains("Hi there!"));
    }
}
