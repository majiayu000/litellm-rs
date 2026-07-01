//! Azure Response Processor

use super::{AzureProcessedResponse, AzureResponseMetadata, ResponseMetrics};
use serde::{Deserialize, Serialize};

/// Configuration for response processing
#[derive(Debug, Clone)]
pub struct ResponseProcessingConfig {
    /// Extract and validate content filters
    pub process_content_filters: bool,
    /// Calculate detailed metrics
    pub calculate_metrics: bool,
    /// Validate response structure
    pub validate_structure: bool,
    /// Maximum response size to process (bytes)
    pub max_response_size: usize,
}

impl Default for ResponseProcessingConfig {
    fn default() -> Self {
        Self {
            process_content_filters: true,
            calculate_metrics: true,
            validate_structure: true,
            max_response_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

/// Main Azure response processor
pub struct AzureResponseProcessor {
    config: ResponseProcessingConfig,
}

impl AzureResponseProcessor {
    pub fn new() -> Self {
        Self {
            config: ResponseProcessingConfig::default(),
        }
    }

    pub fn with_config(config: ResponseProcessingConfig) -> Self {
        Self { config }
    }

    /// Process any Azure response with metadata extraction
    pub fn process_response<T: Serialize + for<'de> Deserialize<'de>>(
        &self,
        response: T,
    ) -> Result<AzureProcessedResponse<T>, String> {
        let start_time = std::time::Instant::now();

        // Serialize to JSON for processing
        let json_response = serde_json::to_value(&response)
            .map_err(|e| format!("Failed to serialize response: {}", e))?;

        // Check response size
        let response_size = serde_json::to_vec(&response).map_or(0, |v| v.len());
        if response_size > self.config.max_response_size {
            return Err(format!(
                "Response size {} exceeds limit of {}",
                response_size, self.config.max_response_size
            ));
        }

        // Validate structure if enabled
        if self.config.validate_structure {
            self.validate_response_structure(&json_response)?;
        }

        // Extract metadata
        let metadata = self.extract_metadata(&json_response);

        // Check content filtering
        let content_filtered = if self.config.process_content_filters {
            self.check_content_filtering(&json_response)
        } else {
            false
        };

        // Collect warnings
        let warnings = self.collect_warnings(&json_response);

        // Calculate metrics
        let metrics = if self.config.calculate_metrics {
            self.calculate_metrics(&json_response, start_time, response_size)
        } else {
            ResponseMetrics::default()
        };

        Ok(AzureProcessedResponse {
            data: response,
            metadata,
            content_filtered,
            warnings,
            metrics,
        })
    }

    /// Process streaming response chunk
    pub fn process_streaming_chunk<T: Serialize>(
        &self,
        chunk: T,
        is_final: bool,
    ) -> Result<StreamingChunk, String> {
        let json_chunk = serde_json::to_value(&chunk)
            .map_err(|e| format!("Failed to serialize chunk: {}", e))?;

        let content_filtered = self.check_content_filtering_chunk(&json_chunk);
        let warnings = self.collect_chunk_warnings(&json_chunk);

        Ok(StreamingChunk {
            data: json_chunk,
            is_final,
            content_filtered,
            warnings,
        })
    }

    /// Validate response has expected structure
    fn validate_response_structure(&self, response: &serde_json::Value) -> Result<(), String> {
        // Check for required fields based on response type

        // Chat completion validation
        if response.get("choices").is_some() {
            self.validate_chat_completion_structure(response)?;
        }
        // Embedding validation
        else if response.get("data").is_some() {
            self.validate_embedding_structure(response)?;
        }
        // Image generation validation
        else if response.get("created").is_some() && response.get("data").is_some() {
            self.validate_image_generation_structure(response)?;
        }

        Ok(())
    }

    fn validate_chat_completion_structure(
        &self,
        response: &serde_json::Value,
    ) -> Result<(), String> {
        let choices = response
            .get("choices")
            .and_then(|c| c.as_array())
            .ok_or("Invalid choices array")?;

        if choices.is_empty() {
            return Err("Empty choices array".to_string());
        }

        // Validate first choice structure
        let first_choice = &choices[0];

        // Should have either message (chat) or text (completion)
        if first_choice.get("message").is_none() && first_choice.get("text").is_none() {
            return Err("Choice missing message or text content".to_string());
        }

        // Should have finish_reason
        if first_choice.get("finish_reason").is_none() {
            return Err("Choice missing finish_reason".to_string());
        }

        Ok(())
    }

    fn validate_embedding_structure(&self, response: &serde_json::Value) -> Result<(), String> {
        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or("Invalid embedding data array")?;

        if data.is_empty() {
            return Err("Empty embedding data array".to_string());
        }

        // Check first embedding entry
        let first_embedding = &data[0];
        if first_embedding.get("embedding").is_none() {
            return Err("Embedding entry missing embedding field".to_string());
        }

        Ok(())
    }

    fn validate_image_generation_structure(
        &self,
        response: &serde_json::Value,
    ) -> Result<(), String> {
        let data = response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or("Invalid image data array")?;

        if data.is_empty() {
            return Err("Empty image data array".to_string());
        }

        Ok(())
    }

    /// Extract comprehensive metadata from response
    fn extract_metadata(&self, response: &serde_json::Value) -> AzureResponseMetadata {
        let mut metadata = AzureResponseMetadata::default();

        // Extract model info
        if let Some(model) = response.get("model").and_then(|m| m.as_str()) {
            metadata.deployment_id = Some(model.to_string());
        }

        // Extract request ID from headers if available
        if let Some(id) = response.get("id").and_then(|i| i.as_str()) {
            metadata.request_id = Some(id.to_string());
        }

        // Extract content filter results
        metadata.content_filter_results = self.extract_content_filters(response);

        // Extract prompt filter results
        metadata.prompt_filter_results = self.extract_prompt_filters(response);

        metadata
    }

    fn extract_content_filters(
        &self,
        response: &serde_json::Value,
    ) -> Option<super::ContentFilterResults> {
        // Look in choices first
        if let Some(choices) = response.get("choices").and_then(|c| c.as_array())
            && let Some(first_choice) = choices.first()
            && let Some(filters) = first_choice.get("content_filter_results")
            && let Ok(filter_results) = serde_json::from_value(filters.clone())
        {
            return Some(filter_results);
        }

        // Check root level
        if let Some(filters) = response.get("content_filter_results")
            && let Ok(filter_results) = serde_json::from_value(filters.clone())
        {
            return Some(filter_results);
        }

        None
    }

    fn extract_prompt_filters(
        &self,
        response: &serde_json::Value,
    ) -> Option<Vec<super::PromptFilterResult>> {
        if let Some(filters) = response.get("prompt_filter_results")
            && let Ok(filter_results) = serde_json::from_value(filters.clone())
        {
            return Some(filter_results);
        }
        None
    }

    /// Check if content was filtered
    fn check_content_filtering(&self, response: &serde_json::Value) -> bool {
        // Check finish_reason for content_filter
        if let Some(choices) = response.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                if let Some(finish_reason) = choice.get("finish_reason").and_then(|r| r.as_str())
                    && finish_reason == "content_filter"
                {
                    return true;
                }
            }
        }

        // Check content filter results
        if let Some(filters) = self.extract_content_filters(response) {
            return self.is_any_content_filtered(&filters);
        }

        false
    }

    fn check_content_filtering_chunk(&self, chunk: &serde_json::Value) -> bool {
        // Similar to full response but for streaming chunks
        self.check_content_filtering(chunk)
    }

    fn is_any_content_filtered(&self, filters: &super::ContentFilterResults) -> bool {
        filters.hate.as_ref().is_some_and(|f| f.filtered)
            || filters.self_harm.as_ref().is_some_and(|f| f.filtered)
            || filters.sexual.as_ref().is_some_and(|f| f.filtered)
            || filters.violence.as_ref().is_some_and(|f| f.filtered)
    }

    /// Collect processing warnings
    fn collect_warnings(&self, response: &serde_json::Value) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check for unusual response patterns
        if response
            .get("choices")
            .and_then(|c| c.as_array())
            .is_some_and(|arr| arr.is_empty())
        {
            warnings.push("Response contains empty choices array".to_string());
        }

        // Check for missing usage information where expected
        if response.get("choices").is_some() && response.get("usage").is_none() {
            warnings.push("Response missing usage information".to_string());
        }

        // Check for content filtering
        if self.check_content_filtering(response) {
            warnings.push("Content was filtered by Azure content filters".to_string());
        }

        warnings
    }

    fn collect_chunk_warnings(&self, chunk: &serde_json::Value) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.check_content_filtering_chunk(chunk) {
            warnings.push("Streaming chunk was filtered".to_string());
        }

        warnings
    }

    /// Calculate detailed processing metrics
    fn calculate_metrics(
        &self,
        _response: &serde_json::Value,
        start_time: std::time::Instant,
        response_size: usize,
    ) -> ResponseMetrics {
        let total_time = start_time.elapsed().as_millis() as u64;

        ResponseMetrics {
            total_time_ms: total_time,
            transformation_time_ms: total_time / 4, // Rough estimate
            filtering_time_ms: total_time / 8,      // Rough estimate
            response_size_bytes: response_size,
        }
    }
}

impl Default for AzureResponseProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Streaming response chunk
#[derive(Debug, Clone)]
pub struct StreamingChunk {
    pub data: serde_json::Value,
    pub is_final: bool,
    pub content_filtered: bool,
    pub warnings: Vec<String>,
}

#[cfg(test)]
#[path = "processor_tests.rs"]
mod tests;
