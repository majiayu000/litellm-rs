//! Chat request and message types

use super::content::ContentPart;
use super::message::{MessageContent, MessageRole};
use super::thinking::{ThinkingConfig, ThinkingContent};
use super::tools::{FunctionCall, ResponseFormat, Tool, ToolCall, ToolChoice};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stream options for controlling streaming behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOptions {
    /// Whether to include usage information in the stream's final chunk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message role
    pub role: MessageRole,
    /// Message content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    /// Thinking/reasoning content (from thinking-enabled models)
    ///
    /// This field contains the model's thinking process when using:
    /// - OpenAI o1/o3/o4 series (reasoning)
    /// - Anthropic Claude with extended thinking
    /// - DeepSeek R1/Reasoner (reasoning_content)
    /// - Gemini with thinking mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingContent>,
    /// Name of message sender
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tool call list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID for responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Function call (backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
}

impl Default for ChatMessage {
    fn default() -> Self {
        Self {
            role: MessageRole::User,
            content: None,
            thinking: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        }
    }
}

impl ChatMessage {
    /// Check if message has thinking content
    pub fn has_thinking(&self) -> bool {
        self.thinking.is_some()
    }

    /// Get thinking content as text (if available)
    pub fn thinking_text(&self) -> Option<&str> {
        self.thinking.as_ref().and_then(|t| t.as_text())
    }
}

/// Chat request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatRequest {
    /// Model name
    pub model: String,
    /// List of chat messages
    pub messages: Vec<ChatMessage>,
    /// Sampling temperature (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum number of tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Maximum completion tokens (new OpenAI parameter)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// Nucleus sampling parameter (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Frequency penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// Presence penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Enable streaming
    #[serde(default)]
    pub stream: bool,
    /// Stream options (e.g., include_usage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    /// Tool list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Tool selection strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Parallel tool calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Response format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Seed value (for reproducible generation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
    /// Number of choices to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Logit bias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, f32>>,
    /// Legacy function definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<serde_json::Value>>,
    /// Legacy function call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<serde_json::Value>,
    /// Whether to return logprobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    /// Number of top logprobs to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    /// Reasoning effort for OpenAI o-series and GPT-5.x models ("low", "medium", "high")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// Thinking/reasoning configuration
    ///
    /// Enable and configure thinking mode for supported models:
    /// - OpenAI o1/o3/o4 series
    /// - Anthropic Claude with extended thinking
    /// - DeepSeek R1/Reasoner
    /// - Gemini with thinking mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    /// Additional provider-specific parameters
    #[serde(flatten)]
    pub extra_params: HashMap<String, serde_json::Value>,
}

impl ChatRequest {
    /// Create new chat request
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Default::default()
        }
    }

    /// Add message
    pub fn add_message(mut self, role: MessageRole, content: impl Into<MessageContent>) -> Self {
        self.messages.push(ChatMessage {
            role,
            content: Some(content.into()),
            thinking: None,
            ..Default::default()
        });
        self
    }

    /// Add system message
    pub fn add_system_message(self, content: impl Into<String>) -> Self {
        self.add_message(MessageRole::System, MessageContent::Text(content.into()))
    }

    /// Add user message
    pub fn add_user_message(self, content: impl Into<String>) -> Self {
        self.add_message(MessageRole::User, MessageContent::Text(content.into()))
    }

    /// Add assistant message
    pub fn add_assistant_message(self, content: impl Into<String>) -> Self {
        self.add_message(MessageRole::Assistant, MessageContent::Text(content.into()))
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set max tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Enable streaming
    pub fn with_streaming(mut self) -> Self {
        self.stream = true;
        self
    }

    /// Add tools
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Enable thinking/reasoning mode
    ///
    /// # Example
    /// ```rust
    /// # use litellm_rs::core::types::{chat::ChatRequest, thinking::ThinkingConfig};
    /// let request = ChatRequest::new("openrouter/deepseek/deepseek-r1")
    ///     .add_user_message("Solve this problem step by step")
    ///     .with_thinking(ThinkingConfig::high_effort());
    /// ```
    pub fn with_thinking(mut self, thinking: ThinkingConfig) -> Self {
        self.thinking = Some(thinking);
        self
    }

    /// Enable thinking with default configuration
    pub fn enable_thinking(mut self) -> Self {
        self.thinking = Some(ThinkingConfig::medium_effort());
        self
    }

    /// Estimate input token count
    pub fn estimate_input_tokens(&self) -> u32 {
        let mut total = 0;

        for message in &self.messages {
            total += 4; // message structure overhead

            if let Some(content) = &message.content {
                match content {
                    MessageContent::Text(text) => {
                        total += (text.len() as f64 / 4.0).ceil() as u32;
                    }
                    MessageContent::Parts(parts) => {
                        for part in parts {
                            match part {
                                ContentPart::Text { text } => {
                                    total += (text.len() as f64 / 4.0).ceil() as u32;
                                }
                                ContentPart::ImageUrl { .. } | ContentPart::Image { .. } => {
                                    total += 85;
                                }
                                ContentPart::Audio { .. } => {
                                    total += 100;
                                }
                                ContentPart::Document { .. } => {
                                    total += 1000;
                                }
                                ContentPart::ToolResult { .. } => {
                                    total += 50;
                                }
                                ContentPart::ToolUse { .. } => {
                                    total += 100;
                                }
                            }
                        }
                    }
                }
            }
        }

        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_default() {
        let msg = ChatMessage::default();
        assert_eq!(msg.role, MessageRole::User);
        assert!(msg.content.is_none());
        assert!(msg.thinking.is_none());
        assert!(msg.name.is_none());
        assert!(msg.tool_calls.is_none());
    }

    #[test]
    fn test_chat_message_has_thinking() {
        let msg = ChatMessage::default();
        assert!(!msg.has_thinking());
    }

    #[test]
    fn test_chat_request_new() {
        let request = ChatRequest::new("gpt-4");
        assert_eq!(request.model, "gpt-4");
        assert!(request.messages.is_empty());
        assert!(!request.stream);
    }

    #[test]
    fn test_chat_request_add_messages() {
        let request = ChatRequest::new("gpt-4")
            .add_system_message("You are a helpful assistant")
            .add_user_message("Hello")
            .add_assistant_message("Hi there!");

        assert_eq!(request.messages.len(), 3);
        assert_eq!(request.messages[0].role, MessageRole::System);
        assert_eq!(request.messages[1].role, MessageRole::User);
        assert_eq!(request.messages[2].role, MessageRole::Assistant);
    }

    #[test]
    fn test_chat_request_with_temperature() {
        let request = ChatRequest::new("gpt-4").with_temperature(0.7);
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_chat_request_with_max_tokens() {
        let request = ChatRequest::new("gpt-4").with_max_tokens(100);
        assert_eq!(request.max_tokens, Some(100));
    }

    #[test]
    fn test_chat_request_with_streaming() {
        let request = ChatRequest::new("gpt-4").with_streaming();
        assert!(request.stream);
    }

    #[test]
    fn test_chat_request_serialization() {
        let request = ChatRequest::new("gpt-4")
            .add_user_message("Hello")
            .with_temperature(0.5);

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["temperature"], 0.5);
        assert!(json["messages"].is_array());
    }

    #[test]
    fn test_chat_request_estimate_tokens() {
        let request = ChatRequest::new("gpt-4").add_user_message("Hello, world!");

        let tokens = request.estimate_input_tokens();
        assert!(tokens > 0);
    }

    #[test]
    fn test_chat_request_default() {
        let request = ChatRequest::default();
        assert!(request.model.is_empty());
        assert!(request.messages.is_empty());
        assert!(request.temperature.is_none());
        assert!(!request.stream);
    }
}
