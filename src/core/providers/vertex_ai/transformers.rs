//! Request/Response transformers for Vertex AI models

use crate::ProviderError;
use crate::core::providers::google_tool_loop::{
    GoogleToolPlanner, build_tool_config, build_tool_declarations, candidate_index,
    content_has_tool_part, content_has_tool_use, finish_reason, parse_function_call_parts,
};
use crate::core::providers::shared::{
    strict_token_count, strict_usage, strict_vertex_usage_metadata,
};
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
    common_utils::{GenerationConfig, Part, convert_role},
    models::VertexAIModel,
};
use crate::core::providers::gemini::models::{
    has_trailing_assistant_prefill, uses_fixed_sampling_contract,
};

fn vertex_audio_mime_type(format: Option<&str>) -> Result<&'static str, ProviderError> {
    match format {
        Some("aac" | "audio/x-aac") => Ok("audio/x-aac"),
        Some("flac" | "audio/flac") => Ok("audio/flac"),
        Some("mp3" | "audio/mp3") => Ok("audio/mp3"),
        Some("m4a" | "audio/m4a") => Ok("audio/m4a"),
        Some("mpeg" | "audio/mpeg") => Ok("audio/mpeg"),
        Some("mpga" | "audio/mpga") => Ok("audio/mpga"),
        Some("mp4" | "audio/mp4") => Ok("audio/mp4"),
        Some("ogg" | "audio/ogg") => Ok("audio/ogg"),
        Some("pcm" | "audio/pcm") => Ok("audio/pcm"),
        Some("wav" | "audio/wav") => Ok("audio/wav"),
        Some("webm" | "audio/webm") => Ok("audio/webm"),
        Some(format) => Err(ProviderError::invalid_request(
            "vertex_ai",
            format!("Unsupported Vertex audio format: {format}"),
        )),
        None => Err(ProviderError::invalid_request(
            "vertex_ai",
            "Vertex audio content requires a format",
        )),
    }
}

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
        if uses_fixed_sampling_contract(&request.model) && has_trailing_assistant_prefill(request) {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                format!(
                    "Model {} does not accept a trailing non-empty assistant message",
                    request.model
                ),
            ));
        }
        let mut contents = Vec::new();
        let mut system_instruction = None;
        let mut tool_planner = GoogleToolPlanner::new("vertex_ai");

        // Process messages
        for (message_index, message) in request.messages.iter().enumerate() {
            match message.role {
                MessageRole::System => {
                    // Gemini uses system instruction separately
                    if let Some(ref content) = message.content {
                        system_instruction = Some(self.message_content_to_values(content)?);
                    }
                }
                _ => {
                    let role = if matches!(message.role, MessageRole::Tool | MessageRole::Function)
                    {
                        "user".to_string()
                    } else {
                        convert_role(&message.role.to_string())
                    };
                    let parts =
                        self.transform_message_content(message_index, message, &mut tool_planner)?;

                    contents.push(json!({ "role": role, "parts": parts }));
                }
            }
        }

        // Build generation config
        let fixed_sampling = uses_fixed_sampling_contract(&request.model);
        let mut generation_config = GenerationConfig {
            temperature: (!fixed_sampling).then_some(request.temperature).flatten(),
            top_p: (!fixed_sampling).then_some(request.top_p).flatten(),
            top_k: None,
            max_output_tokens: request.max_tokens.map(|v| v as i32),
            stop_sequences: request.stop.clone(),
            response_mime_type: None,
            response_schema: None,
        };

        // Handle JSON mode / response format
        if let Some(ref format) = request.response_format
            && format.response_type == Some("json_object".to_string())
        {
            generation_config.response_mime_type = Some("application/json".to_string());
            if let Some(ref schema) = format.json_schema {
                generation_config.response_schema = Some(serde_json::to_value(schema)?);
            }
        }

        let (tools, declaration_names) = build_tool_declarations("vertex_ai", request)?;
        let tool_config = build_tool_config("vertex_ai", request, &declaration_names)?;

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
            body["tools"] = tools;
        }

        if let Some(tool_config) = tool_config {
            body["toolConfig"] = tool_config;
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
                        crate::core::types::content::ContentPart::Audio { audio } => {
                            if audio.data.is_empty() {
                                return Err(ProviderError::invalid_request(
                                    "vertex_ai",
                                    "Audio content cannot be empty",
                                ));
                            }
                            let mime_type = vertex_audio_mime_type(audio.format.as_deref())?;
                            Ok(Part::InlineData {
                                inline_data: super::common_utils::InlineData {
                                    mime_type: mime_type.to_string(),
                                    data: audio.data.clone(),
                                },
                            })
                        }
                        crate::core::types::content::ContentPart::Document { source, .. } => {
                            if source.media_type != "application/pdf" || source.data.is_empty() {
                                return Err(ProviderError::invalid_request(
                                    "vertex_ai",
                                    "Document content requires non-empty base64 application/pdf data",
                                ));
                            }
                            Ok(Part::InlineData {
                                inline_data: super::common_utils::InlineData {
                                    mime_type: source.media_type.clone(),
                                    data: source.data.clone(),
                                },
                            })
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

    fn message_content_to_values(
        &self,
        content: &MessageContent,
    ) -> Result<Vec<Value>, ProviderError> {
        self.message_content_to_parts(content)?
            .into_iter()
            .map(|part| {
                serde_json::to_value(part)
                    .map_err(|error| ProviderError::serialization("vertex_ai", error.to_string()))
            })
            .collect()
    }

    fn transform_message_content(
        &self,
        message_index: usize,
        message: &ChatMessage,
        tool_planner: &mut GoogleToolPlanner,
    ) -> Result<Vec<Value>, ProviderError> {
        if let Some(tool_result) = tool_planner.top_level_result(message)? {
            return Ok(vec![tool_result.to_wire_value()]);
        }
        if message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty())
            && message.role != MessageRole::Assistant
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "tool_calls require assistant role",
            ));
        }
        if message.function_call.is_some() && message.role != MessageRole::Assistant {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "function_call requires assistant role",
            ));
        }
        if message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty())
            && content_has_tool_part(&message.content)
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "tool_calls cannot be combined with tool content parts",
            ));
        }
        if message.function_call.is_some() && content_has_tool_use(&message.content) {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "function_call cannot be combined with tool_use content parts",
            ));
        }

        let mut parts = Vec::new();
        match &message.content {
            Some(MessageContent::Text(text)) => parts.push(json!({ "text": text })),
            Some(MessageContent::Parts(content_parts)) => {
                for part in content_parts {
                    match part {
                        crate::core::types::content::ContentPart::ToolUse { id, name, input } => {
                            if message.role != MessageRole::Assistant {
                                return Err(ProviderError::invalid_request(
                                    "vertex_ai",
                                    "tool_use content requires assistant role",
                                ));
                            }
                            parts.push(
                                tool_planner
                                    .content_tool_use(id, name, input)?
                                    .to_wire_value(),
                            );
                        }
                        crate::core::types::content::ContentPart::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            parts.push(
                                tool_planner
                                    .content_tool_result(tool_use_id, content, *is_error)?
                                    .to_wire_value(),
                            );
                        }
                        _ => {
                            let content = MessageContent::Parts(vec![part.clone()]);
                            parts.extend(self.message_content_to_values(&content)?);
                        }
                    }
                }
            }
            None => {}
        }

        if let Some(tool_calls) = &message.tool_calls {
            for tool_part in tool_planner.top_level_calls(tool_calls)? {
                parts.push(tool_part.to_wire_value());
            }
        }
        if let Some(function_call) = &message.function_call {
            parts.push(
                tool_planner
                    .legacy_function_call(message_index, function_call)?
                    .to_wire_value(),
            );
        }
        if parts.is_empty() {
            parts.push(json!({ "text": "" }));
        }
        Ok(parts)
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

        let mut choices = Vec::with_capacity(candidates.len());
        for (position, candidate) in candidates.iter().enumerate() {
            let parts = candidate
                .get("content")
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ProviderError::response_parsing(
                        "vertex_ai",
                        "Invalid candidate content structure",
                    )
                })?;

            // Extract text from parts
            let mut text_parts = Vec::new();
            for part in parts {
                if let Some(text) = part["text"].as_str() {
                    text_parts.push(text.to_string());
                }
            }
            let index = candidate_index("vertex_ai", candidate, position)?;
            let tool_calls = parse_function_call_parts("vertex_ai", parts, index)?;

            let message_content = if text_parts.is_empty() && !tool_calls.is_empty() {
                None
            } else {
                Some(MessageContent::Text(text_parts.join("")))
            };

            let finish_reason = finish_reason(
                "vertex_ai",
                candidate["finishReason"].as_str(),
                !tool_calls.is_empty(),
            )?;

            choices.push(ChatChoice {
                index,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: message_content,
                    thinking: None,
                    audio: None,
                    name: None,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    function_call: None,
                    tool_call_id: None,
                },
                finish_reason: Some(finish_reason),
                logprobs: None,
            });
        }

        // Parse usage
        let usage = response
            .get("usageMetadata")
            .and_then(strict_vertex_usage_metadata);

        Ok(ChatResponse {
            id: uuid::Uuid::new_v4().to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.model_id(),
            choices,
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

        let usage = response
            .pointer("/metadata/tokenMetadata")
            .and_then(parse_legacy_token_metadata);

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
                    audio: None,
                    name: None,
                    tool_calls: None,
                    function_call: None,
                    tool_call_id: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage,
            system_fingerprint: None,
        })
    }
}

fn parse_legacy_token_metadata(metadata: &Value) -> Option<Usage> {
    let prompt = strict_token_count(metadata.pointer("/inputTokens/totalTokens"))?;
    let completion = strict_token_count(metadata.pointer("/outputTokens/totalTokens"))?;
    strict_usage(&[prompt], &[completion], None, None)
}
#[cfg(test)]
#[path = "transformers/basic_tests.rs"]
mod tests;

#[cfg(test)]
mod split_tests;
