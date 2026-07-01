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
#[path = "utils_tests.rs"]
mod tests;
