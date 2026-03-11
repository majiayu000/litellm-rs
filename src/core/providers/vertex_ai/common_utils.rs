//! Common utilities for Vertex AI

use crate::ProviderError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::models::VertexAIModel;

/// Vertex AI configuration
#[derive(Debug, Clone)]
pub struct VertexAIConfig {
    pub project_id: String,
    pub location: String,
    pub api_version: String,
    pub api_base: Option<String>,
}

/// Check if model supports system messages
pub fn supports_system_messages(model: &VertexAIModel) -> bool {
    model.supports_system_messages()
}

/// Check if model supports response schema
pub fn supports_response_schema(model: &VertexAIModel) -> bool {
    model.supports_response_schema()
}

/// Check if model is global-only (doesn't support regional endpoints)
pub fn is_global_only_model(model: &str) -> bool {
    model.contains("imagen") || model.contains("code-bison") || model.contains("text-bison")
}

/// Vertex AI Content type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

/// Vertex AI Part type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
    FileData { file_data: FileData },
    FunctionCall { function_call: FunctionCall },
    FunctionResponse { function_response: FunctionResponse },
}

/// Inline data for images/media
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineData {
    pub mime_type: String,
    pub data: String, // base64 encoded
}

/// File data reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    pub mime_type: String,
    pub file_uri: String,
}

/// Function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: Value,
}

/// Function response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: Value,
}

/// Generation config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
}

/// Safety settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySettings {
    pub category: String,
    pub threshold: String,
}

/// Tool config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub function_calling_config: FunctionCallingConfig,
}

/// Function calling config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallingConfig {
    pub mode: String, // "AUTO", "ANY", "NONE"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

/// Function declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// Build request body for Gemini models
pub fn build_gemini_request(
    contents: Vec<Content>,
    generation_config: Option<GenerationConfig>,
    safety_settings: Option<Vec<SafetySettings>>,
    tools: Option<Vec<Tool>>,
    tool_config: Option<ToolConfig>,
) -> Value {
    let mut request = serde_json::json!({
        "contents": contents,
    });

    if let Some(config) = generation_config {
        request["generationConfig"] = serde_json::to_value(config).unwrap();
    }

    if let Some(settings) = safety_settings {
        request["safetySettings"] = serde_json::to_value(settings).unwrap();
    }

    if let Some(tools) = tools {
        request["tools"] = serde_json::to_value(tools).unwrap();
    }

    if let Some(tool_config) = tool_config {
        request["toolConfig"] = serde_json::to_value(tool_config).unwrap();
    }

    request
}

/// Convert OpenAI-style role to Vertex AI role
pub fn convert_role(role: &str) -> String {
    match role.to_lowercase().as_str() {
        "system" => "user".to_string(), // Vertex AI doesn't have system role
        "user" => "user".to_string(),
        "assistant" => "model".to_string(),
        "function" => "function".to_string(),
        "tool" => "function".to_string(),
        _ => "user".to_string(),
    }
}

/// Parse safety ratings from response
pub fn parse_safety_ratings(response: &Value) -> Option<Vec<SafetyRating>> {
    response["candidates"]
        .as_array()?
        .first()?
        .get("safetyRatings")?
        .as_array()?
        .iter()
        .filter_map(|rating| {
            Some(SafetyRating {
                category: rating["category"].as_str()?.to_string(),
                probability: rating["probability"].as_str()?.to_string(),
            })
        })
        .collect::<Vec<_>>()
        .into()
}

/// Safety rating
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
}

/// Extract error message from Vertex AI error response
pub fn extract_error_message(response: &Value) -> String {
    if let Some(error) = response.get("error")
        && let Some(message) = error["message"].as_str()
    {
        return message.to_string();
    }

    response.to_string()
}

/// Check if response indicates quota exceeded
pub fn is_quota_exceeded(response: &Value) -> bool {
    if let Some(error) = response.get("error")
        && let Some(message) = error["message"].as_str()
    {
        return message.contains("quota") || message.contains("Quota");
    }
    false
}

/// Check if response indicates rate limit
pub fn is_rate_limited(response: &Value) -> bool {
    if let Some(error) = response.get("error") {
        if let Some(code) = error["code"].as_i64() {
            return code == 429;
        }
        if let Some(message) = error["message"].as_str() {
            return message.contains("rate limit") || message.contains("Rate limit");
        }
    }
    false
}

/// Validate model parameters
pub fn validate_parameters(
    model: &VertexAIModel,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<usize>,
) -> Result<(), ProviderError> {
    // Validate temperature
    if let Some(temp) = temperature
        && !(0.0..=2.0).contains(&temp)
    {
        return Err(ProviderError::invalid_request(
            "vertex_ai",
            format!("Temperature must be between 0.0 and 2.0, got {}", temp),
        ));
    }

    // Validate top_p
    if let Some(p) = top_p
        && !(0.0..=1.0).contains(&p)
    {
        return Err(ProviderError::invalid_request(
            "vertex_ai",
            format!("Top-p must be between 0.0 and 1.0, got {}", p),
        ));
    }

    // Validate max_tokens against model limits
    if let Some(max) = max_tokens {
        let model_max = model.max_context_tokens();
        if max > model_max {
            return Err(ProviderError::context_length_exceeded(
                "vertex_ai",
                model_max,
                max,
            ));
        }
    }

    Ok(())
}

/// Build URL for Vertex AI endpoint
pub fn build_vertex_url(
    config: &VertexAIConfig,
    model: &str,
    endpoint: &str,
    stream: bool,
) -> String {
    let base = if let Some(ref custom_base) = config.api_base {
        custom_base.clone()
    } else if config.location == "global" || is_global_only_model(model) {
        format!(
            "https://aiplatform.googleapis.com/{}/projects/{}/locations/global",
            config.api_version, config.project_id
        )
    } else {
        format!(
            "https://{}-aiplatform.googleapis.com/{}/projects/{}/locations/{}",
            config.location, config.api_version, config.project_id, config.location
        )
    };

    let url = format!("{}/publishers/google/models/{}:{}", base, model, endpoint);

    if stream {
        format!("{}?alt=sse", url)
    } else {
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_role() {
        assert_eq!(convert_role("system"), "user");
        assert_eq!(convert_role("user"), "user");
        assert_eq!(convert_role("assistant"), "model");
        assert_eq!(convert_role("function"), "function");
    }

    #[test]
    fn test_is_global_only_model() {
        assert!(is_global_only_model("imagen-2"));
        assert!(is_global_only_model("code-bison"));
        assert!(!is_global_only_model("gemini-pro"));
    }

    #[test]
    fn test_validate_parameters() {
        let model = VertexAIModel::GeminiPro;

        // Valid parameters
        assert!(validate_parameters(&model, Some(0.7), Some(0.9), Some(1000)).is_ok());

        // Invalid temperature
        assert!(validate_parameters(&model, Some(3.0), None, None).is_err());

        // Invalid top_p
        assert!(validate_parameters(&model, None, Some(1.5), None).is_err());
    }
}
