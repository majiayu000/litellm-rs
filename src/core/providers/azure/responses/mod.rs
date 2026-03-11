//! Azure OpenAI Response Processing Module
//!
//! Specialized response transformation and processing for Azure OpenAI

pub mod o_series_transformation;
pub mod processor;
pub mod transformation;
pub mod utils;

// Re-export main components
pub use o_series_transformation::{OSeriesResponseProcessor, OSeriesResponseTransformation};
pub use processor::{AzureResponseProcessor, ResponseProcessingConfig};
pub use transformation::{AzureResponseTransformation, ResponseTransformConfig};
pub use utils::AzureResponseUtils;

use serde::{Deserialize, Serialize};

/// Azure-specific response metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AzureResponseMetadata {
    /// Content filter results
    pub content_filter_results: Option<ContentFilterResults>,
    /// Prompt filter results  
    pub prompt_filter_results: Option<Vec<PromptFilterResult>>,
    /// Azure region information
    pub region: Option<String>,
    /// Model deployment information
    pub deployment_id: Option<String>,
    /// Request ID for tracking
    pub request_id: Option<String>,
    /// Processing time in milliseconds
    pub processing_time_ms: Option<u64>,
}

/// Content filtering results from Azure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilterResults {
    pub hate: Option<ContentFilterSeverity>,
    pub self_harm: Option<ContentFilterSeverity>,
    pub sexual: Option<ContentFilterSeverity>,
    pub violence: Option<ContentFilterSeverity>,
    pub error: Option<ContentFilterError>,
}

/// Content filter severity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilterSeverity {
    pub filtered: bool,
    pub severity: String,
}

/// Content filter error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilterError {
    pub code: String,
    pub message: String,
}

/// Prompt filter results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFilterResult {
    pub prompt_index: u32,
    pub content_filter_results: ContentFilterResults,
}

/// Azure response processing result
#[derive(Debug, Clone)]
pub struct AzureProcessedResponse<T> {
    /// Original response data
    pub data: T,
    /// Azure-specific metadata
    pub metadata: AzureResponseMetadata,
    /// Whether content was filtered
    pub content_filtered: bool,
    /// Processing warnings
    pub warnings: Vec<String>,
    /// Performance metrics
    pub metrics: ResponseMetrics,
}

/// Response processing metrics
#[derive(Debug, Clone, Default)]
pub struct ResponseMetrics {
    /// Total processing time
    pub total_time_ms: u64,
    /// Time spent on transformations
    pub transformation_time_ms: u64,
    /// Time spent on filtering
    pub filtering_time_ms: u64,
    /// Response size in bytes
    pub response_size_bytes: usize,
}

/// Main Azure response handler
pub struct AzureResponseHandler {
    processor: AzureResponseProcessor,
    _transformation: AzureResponseTransformation,
    o_series_processor: OSeriesResponseProcessor,
}

impl AzureResponseHandler {
    pub fn new() -> Self {
        Self {
            processor: AzureResponseProcessor::new(),
            _transformation: AzureResponseTransformation::new(),
            o_series_processor: OSeriesResponseProcessor::new(),
        }
    }

    pub fn with_config(
        processing_config: ResponseProcessingConfig,
        transform_config: ResponseTransformConfig,
    ) -> Self {
        Self {
            processor: AzureResponseProcessor::with_config(processing_config),
            _transformation: AzureResponseTransformation::with_config(transform_config),
            o_series_processor: OSeriesResponseProcessor::new(),
        }
    }

    /// Process any Azure response
    pub fn process_response<T: Serialize + for<'de> Deserialize<'de> + Clone>(
        &self,
        response: T,
        model: Option<&str>,
    ) -> Result<AzureProcessedResponse<T>, String> {
        let start_time = std::time::Instant::now();

        // Check if this is an O-series model response
        let is_o_series = model.is_some_and(|m| {
            m.to_lowercase().contains("o1") || m.to_lowercase().contains("reasoning")
        });

        let processed = if is_o_series {
            self.o_series_processor.process_response(response)?
        } else {
            self.processor.process_response(response)?
        };

        let total_time = start_time.elapsed().as_millis() as u64;

        // Add timing metrics
        let mut result = processed;
        result.metrics.total_time_ms = total_time;

        Ok(result)
    }

    /// Extract content filtering information
    pub fn extract_content_filters<T>(&self, response: &T) -> Option<ContentFilterResults>
    where
        T: Serialize,
    {
        // Convert to JSON for processing
        if let Ok(json) = serde_json::to_value(response) {
            return self.extract_filters_from_json(&json);
        }
        None
    }

    /// Check if response was filtered
    pub fn is_content_filtered<T>(&self, response: &T) -> bool
    where
        T: Serialize,
    {
        if let Some(filters) = self.extract_content_filters(response) {
            return self.check_any_filtered(&filters);
        }
        false
    }

    /// Get response statistics
    pub fn get_response_stats<T>(&self, response: &T) -> ResponseStats
    where
        T: Serialize,
    {
        let json_size = serde_json::to_vec(response).map_or(0, |v| v.len());

        ResponseStats {
            size_bytes: json_size,
            has_content_filters: self.extract_content_filters(response).is_some(),
            is_filtered: self.is_content_filtered(response),
            estimated_tokens: self.estimate_response_tokens(response),
        }
    }

    // Private helper methods
    fn extract_filters_from_json(&self, json: &serde_json::Value) -> Option<ContentFilterResults> {
        // Look for content filter results in various locations
        if let Some(choices) = json.get("choices").and_then(|c| c.as_array())
            && let Some(first_choice) = choices.first()
            && let Some(filters) = first_choice.get("content_filter_results")
        {
            return serde_json::from_value(filters.clone()).ok();
        }

        // Check root level
        if let Some(filters) = json.get("content_filter_results") {
            return serde_json::from_value(filters.clone()).ok();
        }

        None
    }

    fn check_any_filtered(&self, filters: &ContentFilterResults) -> bool {
        filters.hate.as_ref().is_some_and(|f| f.filtered)
            || filters.self_harm.as_ref().is_some_and(|f| f.filtered)
            || filters.sexual.as_ref().is_some_and(|f| f.filtered)
            || filters.violence.as_ref().is_some_and(|f| f.filtered)
    }

    fn estimate_response_tokens<T>(&self, response: &T) -> u32
    where
        T: Serialize,
    {
        // Rough estimation based on JSON serialization
        if let Ok(json_str) = serde_json::to_string(response) {
            return (json_str.len() as f32 / 4.0).ceil() as u32;
        }
        0
    }
}

impl Default for AzureResponseHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Response statistics
#[derive(Debug, Clone)]
pub struct ResponseStats {
    pub size_bytes: usize,
    pub has_content_filters: bool,
    pub is_filtered: bool,
    pub estimated_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_handler_creation() {
        let handler = AzureResponseHandler::new();
        assert!(!handler.is_content_filtered(&serde_json::json!({"test": "value"})));
    }

    #[test]
    fn test_content_filter_detection() {
        let response = serde_json::json!({
            "choices": [{
                "content_filter_results": {
                    "hate": {"filtered": true, "severity": "medium"}
                }
            }]
        });

        let handler = AzureResponseHandler::new();
        assert!(handler.is_content_filtered(&response));
    }
}
