//! Request/Response transformers for Vertex AI models

use crate::ProviderError;
use crate::core::types::responses::FinishReason;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    message::MessageContent,
    message::MessageRole,
    responses::{ChatChoice, ChatResponse, Usage},
};
use serde_json::{Value, json};

use super::{
    common_utils::{Content, FunctionDeclaration, GenerationConfig, Part, Tool, convert_role},
    models::VertexAIModel,
};

/// Transformer for Gemini models
#[derive(Debug, Clone, Default)]
pub struct GeminiTransformer;

impl GeminiTransformer {
    pub fn new() -> Self {
        Self
    }

    /// Transform chat request to Gemini format
    pub fn transform_chat_request(
        &self,
        request: &ChatRequest,
        _model: &VertexAIModel,
    ) -> Result<Value, ProviderError> {
        let mut contents = Vec::new();
        let mut system_instruction = None;

        // Process messages
        for message in &request.messages {
            match message.role {
                MessageRole::System => {
                    // Gemini uses system instruction separately
                    if let Some(ref content) = message.content {
                        system_instruction = Some(self.message_content_to_parts(content)?);
                    }
                }
                _ => {
                    let role = convert_role(&message.role.to_string());
                    let parts = if let Some(ref content) = message.content {
                        self.message_content_to_parts(content)?
                    } else {
                        vec![]
                    };

                    contents.push(Content { role, parts });
                }
            }
        }

        // Build generation config
        let mut generation_config = GenerationConfig {
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: None,
            max_output_tokens: request.max_tokens.map(|v| v as i32),
            stop_sequences: request.stop.clone(),
            response_mime_type: None,
            response_schema: None,
        };

        // Handle JSON mode / response format
        if let Some(ref format) = request.response_format {
            if format.response_type == Some("json_object".to_string()) {
                generation_config.response_mime_type = Some("application/json".to_string());
                if let Some(ref schema) = format.json_schema {
                    generation_config.response_schema = Some(serde_json::to_value(schema)?);
                }
            }
        }

        // Handle tools/functions
        let tools = if let Some(ref tools) = request.tools {
            Some(vec![Tool {
                function_declarations: tools
                    .iter()
                    .map(|tool| FunctionDeclaration {
                        name: tool.function.name.clone(),
                        description: tool.function.description.clone().unwrap_or_default(),
                        parameters: tool.function.parameters.clone().unwrap_or(json!({})),
                    })
                    .collect(),
            }])
        } else {
            None
        };

        // Build request body
        let mut body = json!({
            "contents": contents,
            "generationConfig": generation_config,
        });

        if let Some(system) = system_instruction {
            body["systemInstruction"] = json!({
                "parts": system
            });
        }

        if let Some(tools) = tools {
            body["tools"] = serde_json::to_value(tools)?;
        }

        Ok(body)
    }

    /// Convert message content to Gemini parts
    fn message_content_to_parts(
        &self,
        content: &MessageContent,
    ) -> Result<Vec<Part>, ProviderError> {
        match content {
            MessageContent::Text(text) => Ok(vec![Part::Text { text: text.clone() }]),
            MessageContent::Parts(parts) => {
                parts.iter().map(|part| {
                    match part {
                        crate::core::types::content::ContentPart::Text { text } => {
                            Ok(Part::Text { text: text.clone() })
                        }
                        crate::core::types::content::ContentPart::Image { image_url, source: _source, detail: _detail } => {
                            // Parse image URL - could be base64 or URL
                            if let Some(url) = &image_url.as_ref().map(|u| &u.url) {
                                if let Some(base64_data) = url.strip_prefix("data:") {
                                    let parts: Vec<&str> = base64_data.splitn(2, ',').collect();
                                    if parts.len() == 2 {
                                        let mime_type = parts[0].replace(";base64", "");
                                        Ok(Part::InlineData {
                                            inline_data: super::common_utils::InlineData {
                                                mime_type,
                                                data: parts[1].to_string(),
                                            }
                                        })
                                    } else {
                                        Err(ProviderError::invalid_request("vertex_ai", "Invalid base64 image"))
                                    }
                                } else {
                                    // File URL
                                    Ok(Part::FileData {
                                        file_data: super::common_utils::FileData {
                                            mime_type: "image/jpeg".to_string(), // Default
                                            file_uri: url.to_string(),
                                        }
                                    })
                                }
                            } else {
                                Err(ProviderError::invalid_request("vertex_ai", "Missing image URL"))
                            }
                        }
                        crate::core::types::content::ContentPart::ImageUrl { image_url } => {
                            // Handle ImageUrl variant
                            if let Some(base64_data) = image_url.url.strip_prefix("data:") {
                                let parts: Vec<&str> = base64_data.splitn(2, ',').collect();
                                if parts.len() == 2 {
                                    let mime_type = parts[0].replace(";base64", "");
                                    Ok(Part::InlineData {
                                        inline_data: crate::core::providers::vertex_ai::common_utils::InlineData {
                                            mime_type,
                                            data: parts[1].to_string(),
                                        },
                                    })
                                } else {
                                    Err(ProviderError::invalid_request("vertex_ai", "Invalid base64 format"))
                                }
                            } else {
                                Err(ProviderError::invalid_request("vertex_ai", "Only base64 images supported"))
                            }
                        }
                        crate::core::types::content::ContentPart::Audio { audio: _audio } => {
                            // Vertex AI doesn't directly support audio in chat completions
                            // This would need to be handled via separate audio APIs
                            Err(ProviderError::invalid_request("vertex_ai", "Audio content not supported in chat completions"))
                        }
                        crate::core::types::content::ContentPart::Document { .. } => {
                            Err(ProviderError::invalid_request("vertex_ai", "Document content not supported"))
                        }
                        crate::core::types::content::ContentPart::ToolResult { .. } => {
                            Err(ProviderError::invalid_request("vertex_ai", "ToolResult should be handled separately"))
                        }
                        crate::core::types::content::ContentPart::ToolUse { .. } => {
                            Err(ProviderError::invalid_request("vertex_ai", "ToolUse should be handled separately"))
                        }
                    }
                }).collect()
            }
        }
    }

    /// Transform Gemini response to standard format
    pub fn transform_chat_response(
        &self,
        response: Value,
        model: &VertexAIModel,
    ) -> Result<ChatResponse, ProviderError> {
        let candidates = response["candidates"]
            .as_array()
            .ok_or_else(|| ProviderError::response_parsing("vertex_ai", "Missing candidates"))?;

        if candidates.is_empty() {
            return Err(ProviderError::response_parsing(
                "vertex_ai",
                "No candidates in response",
            ));
        }

        let candidate = &candidates[0];
        let content = &candidate["content"];

        // Extract text from parts
        let mut text_parts = Vec::new();
        if let Some(parts) = content["parts"].as_array() {
            for part in parts {
                if let Some(text) = part["text"].as_str() {
                    text_parts.push(text.to_string());
                }
            }
        }

        let message_content = if text_parts.is_empty() {
            None
        } else {
            Some(MessageContent::Text(text_parts.join("")))
        };

        // Parse finish reason
        let finish_reason = candidate["finishReason"]
            .as_str()
            .map(|reason| match reason {
                "STOP" => FinishReason::Stop,
                "MAX_TOKENS" => FinishReason::Length,
                "SAFETY" => FinishReason::ContentFilter,
                "RECITATION" => FinishReason::ContentFilter,
                _ => FinishReason::Stop,
            });

        // Parse usage
        let usage = response.get("usageMetadata").map(|usage_metadata| Usage {
            prompt_tokens: usage_metadata["promptTokenCount"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage_metadata["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage_metadata["totalTokenCount"].as_u64().unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        Ok(ChatResponse {
            id: uuid::Uuid::new_v4().to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.model_id(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: message_content,
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    function_call: None,
                    tool_call_id: None,
                },
                finish_reason,
                logprobs: None,
            }],
            usage,
            system_fingerprint: None,
        })
    }
}

/// Transformer for partner models (Claude, Llama, etc.)
#[derive(Debug, Clone, Default)]
pub struct PartnerModelTransformer;

impl PartnerModelTransformer {
    pub fn new() -> Self {
        Self
    }

    /// Transform chat request for partner models
    pub fn transform_chat_request(
        &self,
        request: &ChatRequest,
        model: &VertexAIModel,
    ) -> Result<Value, ProviderError> {
        // Partner models use different formats based on the provider
        if model.model_id().contains("claude") {
            self.transform_claude_request(request)
        } else if model.model_id().contains("llama") {
            self.transform_llama_request(request)
        } else if model.model_id().contains("jamba") {
            self.transform_jamba_request(request)
        } else {
            // Default format
            self.transform_default_partner_request(request)
        }
    }

    /// Transform request for Claude models
    fn transform_claude_request(&self, request: &ChatRequest) -> Result<Value, ProviderError> {
        let mut messages = Vec::new();
        let mut system_message = None;

        for message in &request.messages {
            match message.role {
                MessageRole::System => {
                    if let Some(ref content) = message.content {
                        system_message = Some(content.to_string());
                    }
                }
                _ => {
                    messages.push(json!({
                        "role": message.role.to_string().to_lowercase(),
                        "content": message.content.as_ref().map(|c| c.to_string()).unwrap_or_default()
                    }));
                }
            }
        }

        let mut body = json!({
            "anthropic_version": "vertex-2023-10-16",
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
        });

        if let Some(system) = system_message {
            body["system"] = json!(system);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = json!(temp);
        }

        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }

        if let Some(stop) = &request.stop {
            body["stop_sequences"] = json!(stop);
        }

        Ok(json!({
            "instances": [body],
            "parameters": {}
        }))
    }

    /// Transform request for Llama models
    fn transform_llama_request(&self, request: &ChatRequest) -> Result<Value, ProviderError> {
        let prompt = self.messages_to_llama_prompt(&request.messages);

        Ok(json!({
            "instances": [{
                "prompt": prompt,
            }],
            "parameters": {
                "temperature": request.temperature.unwrap_or(0.7),
                "maxOutputTokens": request.max_tokens.unwrap_or(2048),
                "topP": request.top_p.unwrap_or(0.9),
            }
        }))
    }

    /// Transform request for Jamba models
    fn transform_jamba_request(&self, request: &ChatRequest) -> Result<Value, ProviderError> {
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role.to_string().to_lowercase(),
                    "content": msg.content.as_ref().map(|c| c.to_string()).unwrap_or_default()
                })
            })
            .collect();

        Ok(json!({
            "instances": [{
                "messages": messages,
            }],
            "parameters": {
                "temperature": request.temperature.unwrap_or(0.7),
                "max_tokens": request.max_tokens.unwrap_or(4096),
                "top_p": request.top_p.unwrap_or(0.9),
            }
        }))
    }

    /// Default partner model request format
    fn transform_default_partner_request(
        &self,
        request: &ChatRequest,
    ) -> Result<Value, ProviderError> {
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role.to_string().to_lowercase(),
                    "content": msg.content.as_ref().map(|c| c.to_string()).unwrap_or_default()
                })
            })
            .collect();

        Ok(json!({
            "instances": [{
                "messages": messages,
            }],
            "parameters": {
                "temperature": request.temperature,
                "maxOutputTokens": request.max_tokens,
                "topP": request.top_p,
            }
        }))
    }

    /// Convert messages to Llama prompt format
    fn messages_to_llama_prompt(&self, messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();

        for message in messages {
            let content = message
                .content
                .as_ref()
                .map(|c| c.to_string())
                .unwrap_or_default();
            match message.role {
                MessageRole::System => {
                    prompt.push_str(&format!("<<SYS>>\n{}\n<</SYS>>\n\n", content));
                }
                MessageRole::User => {
                    prompt.push_str(&format!("[INST] {} [/INST]", content));
                }
                MessageRole::Assistant => {
                    prompt.push_str(&format!(" {}", content));
                }
                _ => {}
            }
        }

        prompt
    }

    /// Transform partner model response to standard format
    pub fn transform_chat_response(
        &self,
        response: Value,
        model: &VertexAIModel,
    ) -> Result<ChatResponse, ProviderError> {
        let predictions = response["predictions"]
            .as_array()
            .ok_or_else(|| ProviderError::response_parsing("vertex_ai", "Missing predictions"))?;

        if predictions.is_empty() {
            return Err(ProviderError::response_parsing(
                "vertex_ai",
                "No predictions in response",
            ));
        }

        let prediction = &predictions[0];

        // Extract content based on model type
        let content = if model.model_id().contains("claude") {
            prediction["content"]
                .as_str()
                .or_else(|| prediction["completion"].as_str())
                .map(|s| s.to_string())
        } else {
            prediction["content"]
                .as_str()
                .or_else(|| prediction["text"].as_str())
                .or_else(|| prediction["output"].as_str())
                .map(|s| s.to_string())
        };

        let message_content = content.map(MessageContent::Text);

        // Try to extract usage if available
        let usage = if let Some(metadata) = response.get("metadata") {
            metadata.get("tokenMetadata").map(|token_metadata| Usage {
                prompt_tokens: token_metadata["inputTokens"]["totalTokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
                completion_tokens: token_metadata["outputTokens"]["totalTokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
                total_tokens: 0, // Will be calculated
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            })
        } else {
            None
        };

        let mut usage = usage.unwrap_or(Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        if usage.total_tokens == 0 {
            usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
        }

        Ok(ChatResponse {
            id: uuid::Uuid::new_v4().to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.model_id(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: message_content,
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    function_call: None,
                    tool_call_id: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(usage),
            system_fingerprint: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::content::ContentPart;

    fn create_test_message(role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: Some(MessageContent::Text(content.to_string())),
            thinking: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn create_test_request() -> ChatRequest {
        ChatRequest {
            model: "gemini-1.5-pro".to_string(),
            messages: vec![create_test_message(MessageRole::User, "Hello")],
            ..Default::default()
        }
    }

    // ==================== GeminiTransformer Tests ====================

    #[test]
    fn test_gemini_transformer_new() {
        let transformer = GeminiTransformer::new();
        assert!(format!("{:?}", transformer).contains("GeminiTransformer"));
    }

    #[test]
    fn test_gemini_transformer_default() {
        let transformer = GeminiTransformer;
        assert!(format!("{:?}", transformer).contains("GeminiTransformer"));
    }

    #[test]
    fn test_gemini_transformer_clone() {
        let transformer = GeminiTransformer::new();
        let cloned = transformer.clone();
        assert!(format!("{:?}", cloned).contains("GeminiTransformer"));
    }

    #[test]
    fn test_transform_chat_request_basic() {
        let transformer = GeminiTransformer::new();
        let request = create_test_request();
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body["contents"].is_array());
        assert!(body["generationConfig"].is_object());
    }

    #[test]
    fn test_transform_chat_request_with_system_message() {
        let transformer = GeminiTransformer::new();
        let request = ChatRequest {
            model: "gemini-1.5-pro".to_string(),
            messages: vec![
                create_test_message(MessageRole::System, "You are helpful"),
                create_test_message(MessageRole::User, "Hello"),
            ],
            ..Default::default()
        };
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body["systemInstruction"].is_object());
        assert!(body["systemInstruction"]["parts"].is_array());
    }

    #[test]
    fn test_transform_chat_request_with_temperature() {
        let transformer = GeminiTransformer::new();
        let mut request = create_test_request();
        request.temperature = Some(0.7);
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!((body["generationConfig"]["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_transform_chat_request_with_max_tokens() {
        let transformer = GeminiTransformer::new();
        let mut request = create_test_request();
        request.max_tokens = Some(1000);
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert_eq!(body["generationConfig"]["max_output_tokens"], 1000);
    }

    #[test]
    fn test_transform_chat_request_with_top_p() {
        let transformer = GeminiTransformer::new();
        let mut request = create_test_request();
        request.top_p = Some(0.9);
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!((body["generationConfig"]["top_p"].as_f64().unwrap() - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_transform_chat_request_with_stop_sequences() {
        let transformer = GeminiTransformer::new();
        let mut request = create_test_request();
        request.stop = Some(vec!["END".to_string(), "STOP".to_string()]);
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        let stop_seqs = body["generationConfig"]["stop_sequences"]
            .as_array()
            .unwrap();
        assert_eq!(stop_seqs.len(), 2);
    }

    #[test]
    fn test_transform_chat_request_multi_turn() {
        let transformer = GeminiTransformer::new();
        let request = ChatRequest {
            model: "gemini-1.5-pro".to_string(),
            messages: vec![
                create_test_message(MessageRole::User, "Hello"),
                create_test_message(MessageRole::Assistant, "Hi there!"),
                create_test_message(MessageRole::User, "How are you?"),
            ],
            ..Default::default()
        };
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 3);
    }

    #[test]
    fn test_transform_chat_response_basic() {
        let transformer = GeminiTransformer::new();
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello! How can I help?"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 20,
                "totalTokenCount": 30
            }
        });
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_response(response, &model);
        assert!(result.is_ok());
        let chat_response = result.unwrap();
        assert_eq!(chat_response.object, "chat.completion");
        assert_eq!(chat_response.choices.len(), 1);
        assert_eq!(
            chat_response.choices[0].finish_reason,
            Some(FinishReason::Stop)
        );
    }

    #[test]
    fn test_transform_chat_response_finish_reasons() {
        let transformer = GeminiTransformer::new();
        let model = VertexAIModel::GeminiPro;

        // Test STOP
        let response = json!({
            "candidates": [{"content": {"parts": [{"text": "Done"}]}, "finishReason": "STOP"}]
        });
        let result = transformer
            .transform_chat_response(response, &model)
            .unwrap();
        assert_eq!(result.choices[0].finish_reason, Some(FinishReason::Stop));

        // Test MAX_TOKENS
        let response = json!({
            "candidates": [{"content": {"parts": [{"text": "Done"}]}, "finishReason": "MAX_TOKENS"}]
        });
        let result = transformer
            .transform_chat_response(response, &model)
            .unwrap();
        assert_eq!(result.choices[0].finish_reason, Some(FinishReason::Length));

        // Test SAFETY
        let response = json!({
            "candidates": [{"content": {"parts": [{"text": ""}]}, "finishReason": "SAFETY"}]
        });
        let result = transformer
            .transform_chat_response(response, &model)
            .unwrap();
        assert_eq!(
            result.choices[0].finish_reason,
            Some(FinishReason::ContentFilter)
        );
    }

    #[test]
    fn test_transform_chat_response_with_usage() {
        let transformer = GeminiTransformer::new();
        let response = json!({
            "candidates": [{
                "content": {"parts": [{"text": "Response"}]},
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 100,
                "candidatesTokenCount": 50,
                "totalTokenCount": 150
            }
        });
        let model = VertexAIModel::GeminiPro;

        let result = transformer
            .transform_chat_response(response, &model)
            .unwrap();
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_transform_chat_response_missing_candidates() {
        let transformer = GeminiTransformer::new();
        let response = json!({});
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_response(response, &model);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_chat_response_empty_candidates() {
        let transformer = GeminiTransformer::new();
        let response = json!({"candidates": []});
        let model = VertexAIModel::GeminiPro;

        let result = transformer.transform_chat_response(response, &model);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_content_to_parts_text() {
        let transformer = GeminiTransformer::new();
        let content = MessageContent::Text("Hello world".to_string());

        let result = transformer.message_content_to_parts(&content);
        assert!(result.is_ok());
        let parts = result.unwrap();
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            Part::Text { text } => assert_eq!(text, "Hello world"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_message_content_to_parts_multipart_text() {
        let transformer = GeminiTransformer::new();
        let content = MessageContent::Parts(vec![
            ContentPart::Text {
                text: "Part 1".to_string(),
            },
            ContentPart::Text {
                text: "Part 2".to_string(),
            },
        ]);

        let result = transformer.message_content_to_parts(&content);
        assert!(result.is_ok());
        let parts = result.unwrap();
        assert_eq!(parts.len(), 2);
    }

    // ==================== PartnerModelTransformer Tests ====================

    #[test]
    fn test_partner_transformer_new() {
        let transformer = PartnerModelTransformer::new();
        assert!(format!("{:?}", transformer).contains("PartnerModelTransformer"));
    }

    #[test]
    fn test_partner_transformer_default() {
        let transformer = PartnerModelTransformer;
        assert!(format!("{:?}", transformer).contains("PartnerModelTransformer"));
    }

    #[test]
    fn test_transform_claude_request() {
        let transformer = PartnerModelTransformer::new();
        let request = ChatRequest {
            model: "claude-3-5-sonnet".to_string(),
            messages: vec![
                create_test_message(MessageRole::System, "You are helpful"),
                create_test_message(MessageRole::User, "Hello"),
            ],
            max_tokens: Some(1000),
            temperature: Some(0.7),
            ..Default::default()
        };
        let model = VertexAIModel::Claude35Sonnet;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body["instances"].is_array());
        let instance = &body["instances"][0];
        assert_eq!(instance["anthropic_version"], "vertex-2023-10-16");
        assert!(instance["messages"].is_array());
    }

    #[test]
    fn test_transform_llama_request() {
        let transformer = PartnerModelTransformer::new();
        let request = ChatRequest {
            model: "llama3-70b".to_string(),
            messages: vec![create_test_message(MessageRole::User, "Hello")],
            temperature: Some(0.8),
            max_tokens: Some(500),
            ..Default::default()
        };
        let model = VertexAIModel::Llama3_70B;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body["instances"].is_array());
        assert!(body["instances"][0]["prompt"].is_string());
        assert!(body["parameters"]["temperature"].is_number());
    }

    #[test]
    fn test_transform_jamba_request() {
        let transformer = PartnerModelTransformer::new();
        let request = ChatRequest {
            model: "jamba-1.5-large".to_string(),
            messages: vec![create_test_message(MessageRole::User, "Hello")],
            ..Default::default()
        };
        let model = VertexAIModel::Jamba15Large;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body["instances"].is_array());
        assert!(body["instances"][0]["messages"].is_array());
    }

    #[test]
    fn test_transform_default_partner_request() {
        let transformer = PartnerModelTransformer::new();
        let request = ChatRequest {
            model: "mistral-large".to_string(),
            messages: vec![create_test_message(MessageRole::User, "Hello")],
            ..Default::default()
        };
        let model = VertexAIModel::MistralLarge;

        let result = transformer.transform_chat_request(&request, &model);
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body["instances"].is_array());
    }

    #[test]
    fn test_messages_to_llama_prompt_user_only() {
        let transformer = PartnerModelTransformer::new();
        let messages = vec![create_test_message(MessageRole::User, "Hello")];

        let prompt = transformer.messages_to_llama_prompt(&messages);
        assert!(prompt.contains("[INST] Hello [/INST]"));
    }

    #[test]
    fn test_messages_to_llama_prompt_with_system() {
        let transformer = PartnerModelTransformer::new();
        let messages = vec![
            create_test_message(MessageRole::System, "You are helpful"),
            create_test_message(MessageRole::User, "Hello"),
        ];

        let prompt = transformer.messages_to_llama_prompt(&messages);
        assert!(prompt.contains("<<SYS>>"));
        assert!(prompt.contains("You are helpful"));
        assert!(prompt.contains("<</SYS>>"));
    }

    #[test]
    fn test_messages_to_llama_prompt_conversation() {
        let transformer = PartnerModelTransformer::new();
        let messages = vec![
            create_test_message(MessageRole::User, "Hi"),
            create_test_message(MessageRole::Assistant, "Hello!"),
            create_test_message(MessageRole::User, "How are you?"),
        ];

        let prompt = transformer.messages_to_llama_prompt(&messages);
        assert!(prompt.contains("[INST] Hi [/INST]"));
        assert!(prompt.contains("Hello!"));
        assert!(prompt.contains("[INST] How are you? [/INST]"));
    }

    #[test]
    fn test_transform_partner_response_basic() {
        let transformer = PartnerModelTransformer::new();
        let response = json!({
            "predictions": [{
                "content": "Hello! I'm Claude."
            }]
        });
        let model = VertexAIModel::Claude35Sonnet;

        let result = transformer.transform_chat_response(response, &model);
        assert!(result.is_ok());
        let chat_response = result.unwrap();
        assert_eq!(chat_response.object, "chat.completion");
        assert_eq!(chat_response.choices.len(), 1);
    }

    #[test]
    fn test_transform_partner_response_with_text_field() {
        let transformer = PartnerModelTransformer::new();
        let response = json!({
            "predictions": [{
                "text": "Llama response"
            }]
        });
        let model = VertexAIModel::Llama3_70B;

        let result = transformer.transform_chat_response(response, &model);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform_partner_response_missing_predictions() {
        let transformer = PartnerModelTransformer::new();
        let response = json!({});
        let model = VertexAIModel::Claude35Sonnet;

        let result = transformer.transform_chat_response(response, &model);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_partner_response_empty_predictions() {
        let transformer = PartnerModelTransformer::new();
        let response = json!({"predictions": []});
        let model = VertexAIModel::Claude35Sonnet;

        let result = transformer.transform_chat_response(response, &model);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_partner_response_with_metadata() {
        let transformer = PartnerModelTransformer::new();
        let response = json!({
            "predictions": [{
                "content": "Response"
            }],
            "metadata": {
                "tokenMetadata": {
                    "inputTokens": {"totalTokens": 50},
                    "outputTokens": {"totalTokens": 100}
                }
            }
        });
        let model = VertexAIModel::Claude35Sonnet;

        let result = transformer
            .transform_chat_response(response, &model)
            .unwrap();
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 50);
        assert_eq!(usage.completion_tokens, 100);
        assert_eq!(usage.total_tokens, 150);
    }
}
