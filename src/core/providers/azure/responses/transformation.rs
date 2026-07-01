//! Azure Response Transformation Logic

use serde::Serialize;
use std::collections::HashMap;

/// Configuration for response transformations
#[derive(Debug, Clone)]
pub struct ResponseTransformConfig {
    /// Whether to strip Azure-specific metadata
    pub strip_azure_metadata: bool,
    /// Whether to normalize field names to OpenAI format
    pub normalize_field_names: bool,
    /// Whether to include content filter results
    pub include_content_filters: bool,
    /// Custom field mappings
    pub field_mappings: HashMap<String, String>,
    /// Response format preferences
    pub response_format: ResponseFormat,
}

impl Default for ResponseTransformConfig {
    fn default() -> Self {
        Self {
            strip_azure_metadata: false,
            normalize_field_names: true,
            include_content_filters: true,
            field_mappings: HashMap::new(),
            response_format: ResponseFormat::OpenAICompatible,
        }
    }
}

/// Response format options
#[derive(Debug, Clone, Copy)]
pub enum ResponseFormat {
    /// Keep Azure-specific format
    Native,
    /// Convert to OpenAI-compatible format
    OpenAICompatible,
    /// Minimal response with only essential data
    Minimal,
}

/// Azure response transformation handler
pub struct AzureResponseTransformation {
    config: ResponseTransformConfig,
}

impl AzureResponseTransformation {
    pub fn new() -> Self {
        Self {
            config: ResponseTransformConfig::default(),
        }
    }

    pub fn with_config(config: ResponseTransformConfig) -> Self {
        Self { config }
    }

    /// Transform any Azure response to desired format
    pub fn transform_response<T: Serialize>(
        &self,
        response: T,
    ) -> Result<serde_json::Value, String> {
        let json_response = serde_json::to_value(response)
            .map_err(|e| format!("Failed to serialize response: {}", e))?;

        match self.config.response_format {
            ResponseFormat::Native => Ok(json_response),
            ResponseFormat::OpenAICompatible => self.transform_to_openai_format(json_response),
            ResponseFormat::Minimal => self.transform_to_minimal_format(json_response),
        }
    }

    /// Transform chat completion response
    /// Takes ownership to avoid unnecessary cloning
    pub fn transform_chat_response(
        &self,
        mut response: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Normalize choice structure
        if let Some(choices) = response.get_mut("choices").and_then(|c| c.as_array_mut()) {
            for choice in choices {
                self.transform_choice_object(choice)?;
            }
        }

        // Handle usage information
        if let Some(usage) = response.get_mut("usage") {
            self.transform_usage_object(usage)?;
        }

        // Apply field mappings
        self.apply_field_mappings(&mut response)?;

        // Handle content filters based on config
        if !self.config.include_content_filters {
            self.remove_content_filters(&mut response);
        }

        Ok(response)
    }

    /// Transform completion response
    /// Takes ownership to avoid unnecessary cloning
    pub fn transform_completion_response(
        &self,
        mut response: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Similar transformations as chat but for completion format
        if let Some(choices) = response.get_mut("choices").and_then(|c| c.as_array_mut()) {
            for choice in choices {
                self.transform_completion_choice(choice)?;
            }
        }

        self.apply_field_mappings(&mut response)?;

        if !self.config.include_content_filters {
            self.remove_content_filters(&mut response);
        }

        Ok(response)
    }

    /// Transform embedding response
    /// Takes ownership to avoid unnecessary cloning
    pub fn transform_embedding_response(
        &self,
        mut response: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Embeddings typically don't need much transformation
        // but we can apply field mappings and filter handling

        self.apply_field_mappings(&mut response)?;

        if !self.config.include_content_filters {
            self.remove_content_filters(&mut response);
        }

        Ok(response)
    }

    // Private transformation methods

    fn transform_to_openai_format(
        &self,
        mut response: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Remove Azure-specific fields if requested
        if self.config.strip_azure_metadata {
            self.strip_azure_fields(&mut response);
        }

        // Normalize field names
        if self.config.normalize_field_names {
            self.normalize_fields(&mut response)?;
        }

        Ok(response)
    }

    fn transform_to_minimal_format(
        &self,
        response: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        // Extract only essential fields for minimal response
        let mut minimal = serde_json::json!({});

        // Copy essential fields based on response type
        if let Some(choices) = response.get("choices") {
            minimal["choices"] = choices.clone();
        }

        if let Some(data) = response.get("data") {
            minimal["data"] = data.clone();
        }

        if let Some(usage) = response.get("usage") {
            // Include only token counts
            if let Some(total_tokens) = usage.get("total_tokens") {
                minimal["usage"] = serde_json::json!({
                    "total_tokens": total_tokens
                });
            }
        }

        Ok(minimal)
    }

    fn transform_choice_object(&self, choice: &mut serde_json::Value) -> Result<(), String> {
        // Handle message transformation
        if let Some(message) = choice.get_mut("message") {
            self.transform_message_object(message)?;
        }

        // Handle delta transformation for streaming
        if let Some(delta) = choice.get_mut("delta") {
            self.transform_message_object(delta)?;
        }

        // Transform finish_reason if needed
        if let Some(finish_reason) = choice.get_mut("finish_reason") {
            self.normalize_finish_reason(finish_reason)?;
        }

        Ok(())
    }

    fn transform_completion_choice(&self, choice: &mut serde_json::Value) -> Result<(), String> {
        // Completion choices have simpler structure
        // Mainly just text and finish_reason

        if let Some(finish_reason) = choice.get_mut("finish_reason") {
            self.normalize_finish_reason(finish_reason)?;
        }

        Ok(())
    }

    fn transform_message_object(&self, message: &mut serde_json::Value) -> Result<(), String> {
        // Handle function calls and tool calls normalization
        if let Some(function_call) = message.get_mut("function_call") {
            self.normalize_function_call(function_call)?;
        }

        if let Some(tool_calls) = message.get_mut("tool_calls") {
            self.normalize_tool_calls(tool_calls)?;
        }

        Ok(())
    }

    fn transform_usage_object(&self, usage: &mut serde_json::Value) -> Result<(), String> {
        // Azure might have additional usage fields
        // Normalize to standard OpenAI format if requested

        if self.config.normalize_field_names {
            // Ensure standard field names exist
            if usage.get("prompt_tokens").is_none()
                && usage.get("input_tokens").is_some()
                && let Some(input_tokens) = usage.get("input_tokens").cloned()
            {
                usage["prompt_tokens"] = input_tokens;
                if let Some(usage_obj) = usage.as_object_mut() {
                    usage_obj.remove("input_tokens");
                }
            }

            if usage.get("completion_tokens").is_none()
                && usage.get("output_tokens").is_some()
                && let Some(output_tokens) = usage.get("output_tokens").cloned()
            {
                usage["completion_tokens"] = output_tokens;
                if let Some(usage_obj) = usage.as_object_mut() {
                    usage_obj.remove("output_tokens");
                }
            }
        }

        Ok(())
    }

    fn normalize_finish_reason(&self, finish_reason: &mut serde_json::Value) -> Result<(), String> {
        // Azure might use different finish reason values
        // Normalize to OpenAI standard

        if let Some(reason_str) = finish_reason.as_str() {
            let normalized = match reason_str {
                "content_filter" => "content_filter",
                "max_tokens" => "length",
                "stop_sequence" => "stop",
                _ => reason_str,
            };

            *finish_reason = serde_json::json!(normalized);
        }

        Ok(())
    }

    fn normalize_function_call(
        &self,
        _function_call: &mut serde_json::Value,
    ) -> Result<(), String> {
        // Function call normalization if needed
        Ok(())
    }

    fn normalize_tool_calls(&self, _tool_calls: &mut serde_json::Value) -> Result<(), String> {
        // Tool calls normalization if needed
        Ok(())
    }

    fn apply_field_mappings(&self, response: &mut serde_json::Value) -> Result<(), String> {
        if self.config.field_mappings.is_empty() {
            return Ok(());
        }

        // Apply custom field mappings
        for (from_field, to_field) in &self.config.field_mappings {
            Self::rename_field_recursive(response, from_field, to_field);
        }

        Ok(())
    }

    fn rename_field_recursive(value: &mut serde_json::Value, from_field: &str, to_field: &str) {
        match value {
            serde_json::Value::Object(obj) => {
                // Check if the field exists at this level
                if let Some(field_value) = obj.remove(from_field) {
                    obj.insert(to_field.to_string(), field_value);
                }

                // Recursively process nested objects
                for (_, nested_value) in obj.iter_mut() {
                    Self::rename_field_recursive(nested_value, from_field, to_field);
                }
            }
            serde_json::Value::Array(arr) => {
                // Process array elements
                for item in arr.iter_mut() {
                    Self::rename_field_recursive(item, from_field, to_field);
                }
            }
            _ => {}
        }
    }

    fn remove_content_filters(&self, response: &mut serde_json::Value) {
        Self::remove_field_recursive(response, "content_filter_results");
        Self::remove_field_recursive(response, "prompt_filter_results");
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

    fn strip_azure_fields(&self, response: &mut serde_json::Value) {
        let azure_specific_fields = [
            "deployment_id",
            "azure_endpoint",
            "content_filter_results",
            "prompt_filter_results",
            "region",
        ];

        for field in &azure_specific_fields {
            Self::remove_field_recursive(response, field);
        }
    }

    fn normalize_fields(&self, response: &mut serde_json::Value) -> Result<(), String> {
        // Apply standard OpenAI field normalizations
        let field_mappings = [
            ("input_tokens", "prompt_tokens"),
            ("output_tokens", "completion_tokens"),
        ];

        for (from, to) in &field_mappings {
            Self::rename_field_recursive(response, from, to);
        }

        Ok(())
    }
}

impl Default for AzureResponseTransformation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "transformation_tests.rs"]
mod tests;
