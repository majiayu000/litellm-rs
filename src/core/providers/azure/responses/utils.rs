//! Azure Response Utilities

/// Utilities for processing Azure responses
pub struct AzureResponseUtils;

impl AzureResponseUtils {
    /// Extract response metadata from any Azure response
    pub fn extract_metadata(response: &serde_json::Value) -> ResponseMetadata {
        let mut metadata = ResponseMetadata::default();

        // Extract model information
        if let Some(model) = response.get("model").and_then(|m| m.as_str()) {
            metadata.model = Some(model.to_string());
        }

        // Extract usage information
        if let Some(usage) = response.get("usage") {
            metadata.token_usage = Self::extract_token_usage(usage);
        }

        // Extract timing information
        if let Some(created) = response.get("created").and_then(|c| c.as_u64()) {
            metadata.created_timestamp = Some(created);
        }

        metadata
    }

    /// Extract token usage from usage object
    pub fn extract_token_usage(usage: &serde_json::Value) -> Option<TokenUsage> {
        Some(TokenUsage {
            prompt_tokens: usage
                .get("prompt_tokens")
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: usage
                .get("completion_tokens")
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: usage
                .get("total_tokens")
                .and_then(|t| t.as_u64())
                .unwrap_or(0) as u32,
            reasoning_tokens: usage
                .get("reasoning_tokens")
                .and_then(|t| t.as_u64())
                .map(|t| t as u32),
        })
    }

    /// Check if response indicates content filtering
    pub fn is_content_filtered(response: &serde_json::Value) -> bool {
        // Check choices for content filter results
        if let Some(choices) = response.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(finish_reason) = choice.get("finish_reason").and_then(|r| r.as_str())
                    && finish_reason == "content_filter"
                {
                    return true;
                }

                if let Some(content_filter) = choice.get("content_filter_results")
                    && Self::check_content_filter_object(content_filter)
                {
                    return true;
                }
            }
        }

        // Check root level content filter results
        if let Some(content_filter) = response.get("content_filter_results")
            && Self::check_content_filter_object(content_filter)
        {
            return true;
        }

        false
    }

    /// Extract response content from various response types
    pub fn extract_content(response: &serde_json::Value) -> Option<String> {
        // Try chat completion format first
        if let Some(choices) = response.get("choices").and_then(|c| c.as_array())
            && let Some(first_choice) = choices.first()
        {
            // Chat format
            if let Some(message) = first_choice.get("message")
                && let Some(content) = message.get("content").and_then(|c| c.as_str())
            {
                return Some(content.to_string());
            }

            // Completion format
            if let Some(text) = first_choice.get("text").and_then(|t| t.as_str()) {
                return Some(text.to_string());
            }
        }

        // Try embedding format
        if let Some(data) = response.get("data").and_then(|d| d.as_array()) {
            return Some(format!("Embedding data with {} entries", data.len()));
        }

        None
    }

    /// Extract all choices from response
    pub fn extract_choices(response: &serde_json::Value) -> Vec<ResponseChoice> {
        let mut choices = Vec::new();

        if let Some(response_choices) = response.get("choices").and_then(|c| c.as_array()) {
            for (index, choice) in response_choices.iter().enumerate() {
                choices.push(ResponseChoice {
                    index: index as u32,
                    content: Self::extract_choice_content(choice),
                    finish_reason: choice
                        .get("finish_reason")
                        .and_then(|r| r.as_str())
                        .map(|s| s.to_string()),
                    content_filtered: Self::is_choice_filtered(choice),
                });
            }
        }

        choices
    }

    /// Calculate response statistics
    pub fn calculate_response_stats(response: &serde_json::Value) -> ResponseStats {
        let json_str = serde_json::to_string(response).unwrap_or_default();
        let size_bytes = json_str.len();

        let choices_count = response
            .get("choices")
            .and_then(|c| c.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        let has_function_calls = Self::has_function_calls(response);
        let has_tool_calls = Self::has_tool_calls(response);

        ResponseStats {
            size_bytes,
            choices_count: choices_count as u32,
            has_function_calls,
            has_tool_calls,
            is_streaming: false, // Can't determine from static response
            content_filtered: Self::is_content_filtered(response),
        }
    }

    /// Check if response has function calls
    pub fn has_function_calls(response: &serde_json::Value) -> bool {
        if let Some(choices) = response.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(message) = choice.get("message")
                    && message.get("function_call").is_some()
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check if response has tool calls
    pub fn has_tool_calls(response: &serde_json::Value) -> bool {
        if let Some(choices) = response.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(message) = choice.get("message")
                    && message.get("tool_calls").is_some()
                {
                    return true;
                }
            }
        }
        false
    }

    /// Normalize response for OpenAI compatibility
    pub fn normalize_for_openai(mut response: serde_json::Value) -> serde_json::Value {
        // Remove Azure-specific fields
        Self::remove_azure_specific_fields(&mut response);

        // Normalize field names
        Self::normalize_field_names(&mut response);

        response
    }

    // Private helper methods

    fn check_content_filter_object(content_filter: &serde_json::Value) -> bool {
        if let Some(obj) = content_filter.as_object() {
            for (_, filter_result) in obj {
                if let Some(filtered) = filter_result.get("filtered").and_then(|f| f.as_bool())
                    && filtered
                {
                    return true;
                }
            }
        }
        false
    }

    fn extract_choice_content(choice: &serde_json::Value) -> Option<String> {
        // Try message content first (chat format)
        if let Some(message) = choice.get("message")
            && let Some(content) = message.get("content").and_then(|c| c.as_str())
        {
            return Some(content.to_string());
        }

        // Try text content (completion format)
        if let Some(text) = choice.get("text").and_then(|t| t.as_str()) {
            return Some(text.to_string());
        }

        None
    }

    fn is_choice_filtered(choice: &serde_json::Value) -> bool {
        if let Some(finish_reason) = choice.get("finish_reason").and_then(|r| r.as_str())
            && finish_reason == "content_filter"
        {
            return true;
        }

        if let Some(content_filter) = choice.get("content_filter_results") {
            return Self::check_content_filter_object(content_filter);
        }

        false
    }

    fn remove_azure_specific_fields(response: &mut serde_json::Value) {
        let azure_fields = [
            "content_filter_results",
            "prompt_filter_results",
            "deployment_id",
            "azure_endpoint",
        ];

        for field in &azure_fields {
            Self::remove_field_recursive(response, field);
        }
    }

    fn remove_field_recursive(value: &mut serde_json::Value, field_name: &str) {
        match value {
            serde_json::Value::Object(obj) => {
                obj.remove(field_name);
                for (_, nested_value) in obj.iter_mut() {
                    Self::remove_field_recursive(nested_value, field_name);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::remove_field_recursive(item, field_name);
                }
            }
            _ => {}
        }
    }

    fn normalize_field_names(response: &mut serde_json::Value) {
        let field_mappings = [
            ("input_tokens", "prompt_tokens"),
            ("output_tokens", "completion_tokens"),
        ];

        for (from, to) in &field_mappings {
            Self::rename_field_recursive(response, from, to);
        }
    }

    fn rename_field_recursive(value: &mut serde_json::Value, from: &str, to: &str) {
        match value {
            serde_json::Value::Object(obj) => {
                if let Some(field_value) = obj.remove(from) {
                    obj.insert(to.to_string(), field_value);
                }
                for (_, nested_value) in obj.iter_mut() {
                    Self::rename_field_recursive(nested_value, from, to);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::rename_field_recursive(item, from, to);
                }
            }
            _ => {}
        }
    }
}

/// Response metadata structure
#[derive(Debug, Clone, Default)]
pub struct ResponseMetadata {
    pub model: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub created_timestamp: Option<u64>,
}

/// Token usage information
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: Option<u32>,
}

/// Individual choice information
#[derive(Debug, Clone)]
pub struct ResponseChoice {
    pub index: u32,
    pub content: Option<String>,
    pub finish_reason: Option<String>,
    pub content_filtered: bool,
}

/// Response statistics
#[derive(Debug, Clone)]
pub struct ResponseStats {
    pub size_bytes: usize,
    pub choices_count: u32,
    pub has_function_calls: bool,
    pub has_tool_calls: bool,
    pub is_streaming: bool,
    pub content_filtered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ResponseMetadata Tests ====================

    #[test]
    fn test_response_metadata_default() {
        let metadata = ResponseMetadata::default();

        assert!(metadata.model.is_none());
        assert!(metadata.token_usage.is_none());
        assert!(metadata.created_timestamp.is_none());
    }

    #[test]
    fn test_response_metadata_with_values() {
        let metadata = ResponseMetadata {
            model: Some("gpt-4".to_string()),
            token_usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                reasoning_tokens: None,
            }),
            created_timestamp: Some(1700000000),
        };

        assert_eq!(metadata.model, Some("gpt-4".to_string()));
        assert!(metadata.token_usage.is_some());
        assert_eq!(metadata.created_timestamp, Some(1700000000));
    }

    #[test]
    fn test_response_metadata_clone() {
        let metadata = ResponseMetadata {
            model: Some("gpt-3.5".to_string()),
            token_usage: None,
            created_timestamp: Some(123456),
        };

        let cloned = metadata.clone();
        assert_eq!(cloned.model, metadata.model);
        assert_eq!(cloned.created_timestamp, metadata.created_timestamp);
    }

    #[test]
    fn test_response_metadata_debug() {
        let metadata = ResponseMetadata::default();
        let debug = format!("{:?}", metadata);
        assert!(debug.contains("ResponseMetadata"));
    }

    // ==================== TokenUsage Tests ====================

    #[test]
    fn test_token_usage_creation() {
        let usage = TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            reasoning_tokens: None,
        };

        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert!(usage.reasoning_tokens.is_none());
    }

    #[test]
    fn test_token_usage_with_reasoning() {
        let usage = TokenUsage {
            prompt_tokens: 200,
            completion_tokens: 100,
            total_tokens: 350,
            reasoning_tokens: Some(50),
        };

        assert_eq!(usage.reasoning_tokens, Some(50));
    }

    #[test]
    fn test_token_usage_clone() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            reasoning_tokens: Some(5),
        };

        let cloned = usage.clone();
        assert_eq!(cloned.prompt_tokens, usage.prompt_tokens);
        assert_eq!(cloned.reasoning_tokens, usage.reasoning_tokens);
    }

    // ==================== ResponseChoice Tests ====================

    #[test]
    fn test_response_choice_creation() {
        let choice = ResponseChoice {
            index: 0,
            content: Some("Hello".to_string()),
            finish_reason: Some("stop".to_string()),
            content_filtered: false,
        };

        assert_eq!(choice.index, 0);
        assert_eq!(choice.content, Some("Hello".to_string()));
        assert_eq!(choice.finish_reason, Some("stop".to_string()));
        assert!(!choice.content_filtered);
    }

    #[test]
    fn test_response_choice_filtered() {
        let choice = ResponseChoice {
            index: 0,
            content: None,
            finish_reason: Some("content_filter".to_string()),
            content_filtered: true,
        };

        assert!(choice.content_filtered);
        assert!(choice.content.is_none());
    }

    #[test]
    fn test_response_choice_clone() {
        let choice = ResponseChoice {
            index: 1,
            content: Some("Test".to_string()),
            finish_reason: Some("length".to_string()),
            content_filtered: false,
        };

        let cloned = choice.clone();
        assert_eq!(cloned.index, choice.index);
        assert_eq!(cloned.content, choice.content);
    }

    // ==================== ResponseStats Tests ====================

    #[test]
    fn test_response_stats_creation() {
        let stats = ResponseStats {
            size_bytes: 1024,
            choices_count: 1,
            has_function_calls: false,
            has_tool_calls: false,
            is_streaming: false,
            content_filtered: false,
        };

        assert_eq!(stats.size_bytes, 1024);
        assert_eq!(stats.choices_count, 1);
        assert!(!stats.has_function_calls);
        assert!(!stats.has_tool_calls);
    }

    #[test]
    fn test_response_stats_with_tools() {
        let stats = ResponseStats {
            size_bytes: 2048,
            choices_count: 2,
            has_function_calls: true,
            has_tool_calls: true,
            is_streaming: true,
            content_filtered: false,
        };

        assert!(stats.has_function_calls);
        assert!(stats.has_tool_calls);
        assert!(stats.is_streaming);
    }

    #[test]
    fn test_response_stats_clone() {
        let stats = ResponseStats {
            size_bytes: 512,
            choices_count: 3,
            has_function_calls: true,
            has_tool_calls: false,
            is_streaming: false,
            content_filtered: true,
        };

        let cloned = stats.clone();
        assert_eq!(cloned.size_bytes, stats.size_bytes);
        assert_eq!(cloned.content_filtered, stats.content_filtered);
    }

    // ==================== Extract Metadata Tests ====================

    #[test]
    fn test_extract_metadata_full() {
        let response = serde_json::json!({
            "model": "gpt-4-turbo",
            "created": 1700000000,
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        });

        let metadata = AzureResponseUtils::extract_metadata(&response);
        assert_eq!(metadata.model, Some("gpt-4-turbo".to_string()));
        assert_eq!(metadata.created_timestamp, Some(1700000000));
        assert!(metadata.token_usage.is_some());

        let usage = metadata.token_usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_extract_metadata_minimal() {
        let response = serde_json::json!({});

        let metadata = AzureResponseUtils::extract_metadata(&response);
        assert!(metadata.model.is_none());
        assert!(metadata.created_timestamp.is_none());
    }

    #[test]
    fn test_extract_metadata_partial() {
        let response = serde_json::json!({
            "model": "gpt-3.5-turbo"
        });

        let metadata = AzureResponseUtils::extract_metadata(&response);
        assert_eq!(metadata.model, Some("gpt-3.5-turbo".to_string()));
        assert!(metadata.created_timestamp.is_none());
    }

    // ==================== Extract Token Usage Tests ====================

    #[test]
    fn test_extract_token_usage_full() {
        let usage = serde_json::json!({
            "prompt_tokens": 200,
            "completion_tokens": 100,
            "total_tokens": 300,
            "reasoning_tokens": 50
        });

        let token_usage = AzureResponseUtils::extract_token_usage(&usage).unwrap();
        assert_eq!(token_usage.prompt_tokens, 200);
        assert_eq!(token_usage.completion_tokens, 100);
        assert_eq!(token_usage.total_tokens, 300);
        assert_eq!(token_usage.reasoning_tokens, Some(50));
    }

    #[test]
    fn test_extract_token_usage_no_reasoning() {
        let usage = serde_json::json!({
            "prompt_tokens": 50,
            "completion_tokens": 25,
            "total_tokens": 75
        });

        let token_usage = AzureResponseUtils::extract_token_usage(&usage).unwrap();
        assert_eq!(token_usage.prompt_tokens, 50);
        assert!(token_usage.reasoning_tokens.is_none());
    }

    #[test]
    fn test_extract_token_usage_empty() {
        let usage = serde_json::json!({});

        let token_usage = AzureResponseUtils::extract_token_usage(&usage).unwrap();
        assert_eq!(token_usage.prompt_tokens, 0);
        assert_eq!(token_usage.completion_tokens, 0);
        assert_eq!(token_usage.total_tokens, 0);
    }

    // ==================== Content Filtering Tests ====================

    #[test]
    fn test_is_content_filtered_false() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"content": "Hello"},
                "finish_reason": "stop"
            }]
        });

        assert!(!AzureResponseUtils::is_content_filtered(&response));
    }

    #[test]
    fn test_is_content_filtered_by_finish_reason() {
        let response = serde_json::json!({
            "choices": [{
                "finish_reason": "content_filter"
            }]
        });

        assert!(AzureResponseUtils::is_content_filtered(&response));
    }

    #[test]
    fn test_is_content_filtered_by_filter_results() {
        let response = serde_json::json!({
            "choices": [{
                "finish_reason": "stop",
                "content_filter_results": {
                    "hate": {"filtered": true, "severity": "high"}
                }
            }]
        });

        assert!(AzureResponseUtils::is_content_filtered(&response));
    }

    #[test]
    fn test_is_content_filtered_root_level() {
        let response = serde_json::json!({
            "choices": [{"finish_reason": "stop"}],
            "content_filter_results": {
                "violence": {"filtered": true}
            }
        });

        assert!(AzureResponseUtils::is_content_filtered(&response));
    }

    #[test]
    fn test_is_content_filtered_not_filtered() {
        let response = serde_json::json!({
            "choices": [{
                "finish_reason": "stop",
                "content_filter_results": {
                    "hate": {"filtered": false},
                    "violence": {"filtered": false}
                }
            }]
        });

        assert!(!AzureResponseUtils::is_content_filtered(&response));
    }

    // ==================== Extract Content Tests ====================

    #[test]
    fn test_extract_content_chat_format() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"content": "Hello, world!"}
            }]
        });

        let content = AzureResponseUtils::extract_content(&response);
        assert_eq!(content, Some("Hello, world!".to_string()));
    }

    #[test]
    fn test_extract_content_completion_format() {
        let response = serde_json::json!({
            "choices": [{
                "text": "This is a completion."
            }]
        });

        let content = AzureResponseUtils::extract_content(&response);
        assert_eq!(content, Some("This is a completion.".to_string()));
    }

    #[test]
    fn test_extract_content_embedding_format() {
        let response = serde_json::json!({
            "data": [
                {"embedding": [0.1, 0.2, 0.3]},
                {"embedding": [0.4, 0.5, 0.6]}
            ]
        });

        let content = AzureResponseUtils::extract_content(&response);
        assert!(content.is_some());
        assert!(content.unwrap().contains("2 entries"));
    }

    #[test]
    fn test_extract_content_empty() {
        let response = serde_json::json!({});

        let content = AzureResponseUtils::extract_content(&response);
        assert!(content.is_none());
    }

    #[test]
    fn test_extract_content_empty_choices() {
        let response = serde_json::json!({
            "choices": []
        });

        let content = AzureResponseUtils::extract_content(&response);
        assert!(content.is_none());
    }

    // ==================== Extract Choices Tests ====================

    #[test]
    fn test_extract_choices_single() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"content": "Hello"},
                "finish_reason": "stop"
            }]
        });

        let choices = AzureResponseUtils::extract_choices(&response);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].index, 0);
        assert_eq!(choices[0].content, Some("Hello".to_string()));
        assert_eq!(choices[0].finish_reason, Some("stop".to_string()));
        assert!(!choices[0].content_filtered);
    }

    #[test]
    fn test_extract_choices_multiple() {
        let response = serde_json::json!({
            "choices": [
                {"message": {"content": "Choice 1"}, "finish_reason": "stop"},
                {"message": {"content": "Choice 2"}, "finish_reason": "length"},
                {"message": {"content": ""}, "finish_reason": "content_filter"}
            ]
        });

        let choices = AzureResponseUtils::extract_choices(&response);
        assert_eq!(choices.len(), 3);
        assert_eq!(choices[0].index, 0);
        assert_eq!(choices[1].index, 1);
        assert_eq!(choices[2].index, 2);
        assert!(choices[2].content_filtered);
    }

    #[test]
    fn test_extract_choices_empty() {
        let response = serde_json::json!({
            "choices": []
        });

        let choices = AzureResponseUtils::extract_choices(&response);
        assert!(choices.is_empty());
    }

    #[test]
    fn test_extract_choices_no_choices() {
        let response = serde_json::json!({});

        let choices = AzureResponseUtils::extract_choices(&response);
        assert!(choices.is_empty());
    }

    // ==================== Calculate Response Stats Tests ====================

    #[test]
    fn test_calculate_response_stats_basic() {
        let response = serde_json::json!({
            "choices": [{"message": {"content": "Hello"}, "finish_reason": "stop"}]
        });

        let stats = AzureResponseUtils::calculate_response_stats(&response);
        assert!(stats.size_bytes > 0);
        assert_eq!(stats.choices_count, 1);
        assert!(!stats.has_function_calls);
        assert!(!stats.has_tool_calls);
        assert!(!stats.content_filtered);
    }

    #[test]
    fn test_calculate_response_stats_with_function_call() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "function_call": {"name": "get_weather", "arguments": "{}"}
                },
                "finish_reason": "function_call"
            }]
        });

        let stats = AzureResponseUtils::calculate_response_stats(&response);
        assert!(stats.has_function_calls);
        assert!(!stats.has_tool_calls);
    }

    #[test]
    fn test_calculate_response_stats_with_tool_calls() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{"id": "call_1", "function": {"name": "test"}}]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let stats = AzureResponseUtils::calculate_response_stats(&response);
        assert!(!stats.has_function_calls);
        assert!(stats.has_tool_calls);
    }

    #[test]
    fn test_calculate_response_stats_content_filtered() {
        let response = serde_json::json!({
            "choices": [{"finish_reason": "content_filter"}]
        });

        let stats = AzureResponseUtils::calculate_response_stats(&response);
        assert!(stats.content_filtered);
    }

    #[test]
    fn test_calculate_response_stats_multiple_choices() {
        let response = serde_json::json!({
            "choices": [
                {"message": {"content": "A"}, "finish_reason": "stop"},
                {"message": {"content": "B"}, "finish_reason": "stop"},
                {"message": {"content": "C"}, "finish_reason": "stop"}
            ]
        });

        let stats = AzureResponseUtils::calculate_response_stats(&response);
        assert_eq!(stats.choices_count, 3);
    }

    // ==================== Function/Tool Call Detection Tests ====================

    #[test]
    fn test_has_function_calls_true() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"function_call": {"name": "fn"}}
            }]
        });

        assert!(AzureResponseUtils::has_function_calls(&response));
    }

    #[test]
    fn test_has_function_calls_false() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"content": "Hello"}
            }]
        });

        assert!(!AzureResponseUtils::has_function_calls(&response));
    }

    #[test]
    fn test_has_tool_calls_true() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"tool_calls": []}
            }]
        });

        assert!(AzureResponseUtils::has_tool_calls(&response));
    }

    #[test]
    fn test_has_tool_calls_false() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"content": "Hello"}
            }]
        });

        assert!(!AzureResponseUtils::has_tool_calls(&response));
    }

    // ==================== Normalize for OpenAI Tests ====================

    #[test]
    fn test_normalize_for_openai_removes_azure_fields() {
        let response = serde_json::json!({
            "choices": [{"message": {"content": "Hello"}}],
            "content_filter_results": {"hate": {"filtered": false}},
            "prompt_filter_results": [],
            "deployment_id": "gpt-4"
        });

        let normalized = AzureResponseUtils::normalize_for_openai(response);
        assert!(normalized.get("content_filter_results").is_none());
        assert!(normalized.get("prompt_filter_results").is_none());
        assert!(normalized.get("deployment_id").is_none());
        // Choices should remain
        assert!(normalized.get("choices").is_some());
    }

    #[test]
    fn test_normalize_for_openai_renames_fields() {
        let response = serde_json::json!({
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });

        let normalized = AzureResponseUtils::normalize_for_openai(response);
        let usage = normalized.get("usage").unwrap();
        assert!(usage.get("prompt_tokens").is_some());
        assert!(usage.get("completion_tokens").is_some());
        assert!(usage.get("input_tokens").is_none());
        assert!(usage.get("output_tokens").is_none());
    }

    #[test]
    fn test_normalize_for_openai_nested_azure_fields() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"content": "Test"},
                "content_filter_results": {"hate": {"filtered": false}}
            }]
        });

        let normalized = AzureResponseUtils::normalize_for_openai(response);
        let choices = normalized.get("choices").unwrap().as_array().unwrap();
        let first_choice = &choices[0];
        assert!(first_choice.get("content_filter_results").is_none());
        assert!(first_choice.get("message").is_some());
    }

    #[test]
    fn test_normalize_for_openai_preserves_standard_fields() {
        let response = serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "model": "gpt-4",
            "choices": [{"message": {"content": "Hello"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        });

        let normalized = AzureResponseUtils::normalize_for_openai(response.clone());
        assert_eq!(normalized.get("id"), response.get("id"));
        assert_eq!(normalized.get("object"), response.get("object"));
        assert_eq!(normalized.get("model"), response.get("model"));
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_extract_content_null_content() {
        let response = serde_json::json!({
            "choices": [{
                "message": {"content": null}
            }]
        });

        let content = AzureResponseUtils::extract_content(&response);
        assert!(content.is_none());
    }

    #[test]
    fn test_extract_choices_with_text_format() {
        let response = serde_json::json!({
            "choices": [{
                "text": "Completion text",
                "finish_reason": "stop"
            }]
        });

        let choices = AzureResponseUtils::extract_choices(&response);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].content, Some("Completion text".to_string()));
    }

    #[test]
    fn test_is_content_filtered_empty_choices() {
        let response = serde_json::json!({
            "choices": []
        });

        assert!(!AzureResponseUtils::is_content_filtered(&response));
    }

    #[test]
    fn test_calculate_stats_no_choices() {
        let response = serde_json::json!({
            "data": [{"embedding": [0.1, 0.2]}]
        });

        let stats = AzureResponseUtils::calculate_response_stats(&response);
        assert_eq!(stats.choices_count, 0);
        assert!(!stats.has_function_calls);
        assert!(!stats.has_tool_calls);
    }
}
