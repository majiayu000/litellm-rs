//! Gemini Model Support Module

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Gemini specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    pub model: String,
    pub generation_config: Option<GeminiGenerationConfig>,
    pub safety_settings: Option<Vec<SafetySetting>>,
    pub tools: Option<Vec<Tool>>,
    pub tool_config: Option<ToolConfig>,
    pub system_instruction: Option<SystemInstruction>,
}

/// Gemini generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_mime_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,

    // Gemini 2.0 specific parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_logprobs: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
}

/// Safety settings for Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySetting {
    pub category: SafetyCategory,
    pub threshold: SafetyThreshold,
}

/// Safety categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyCategory {
    #[serde(rename = "HARM_CATEGORY_HARASSMENT")]
    Harassment,
    #[serde(rename = "HARM_CATEGORY_HATE_SPEECH")]
    HateSpeech,
    #[serde(rename = "HARM_CATEGORY_SEXUALLY_EXPLICIT")]
    SexuallyExplicit,
    #[serde(rename = "HARM_CATEGORY_DANGEROUS_CONTENT")]
    DangerousContent,
    #[serde(rename = "HARM_CATEGORY_CIVIC_INTEGRITY")]
    CivicIntegrity,
}

/// Safety thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyThreshold {
    #[serde(rename = "BLOCK_NONE")]
    BlockNone,
    #[serde(rename = "BLOCK_ONLY_HIGH")]
    BlockOnlyHigh,
    #[serde(rename = "BLOCK_MEDIUM_AND_ABOVE")]
    BlockMediumAndAbove,
    #[serde(rename = "BLOCK_LOW_AND_ABOVE")]
    BlockLowAndAbove,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_declarations: Option<Vec<FunctionDeclaration>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_execution: Option<CodeExecution>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_search_retrieval: Option<GoogleSearchRetrieval>,
}

/// Function declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>, // JSON Schema
}

/// Code execution tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecution {}

/// Google Search retrieval tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSearchRetrieval {}

/// Tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub function_calling_config: FunctionCallingConfig,
}

/// Function calling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallingConfig {
    pub mode: FunctionCallingMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

/// Function calling modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FunctionCallingMode {
    #[serde(rename = "AUTO")]
    Auto,
    #[serde(rename = "ANY")]
    Any,
    #[serde(rename = "NONE")]
    None,
}

/// System instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInstruction {
    pub parts: Vec<Part>,
}

/// Content part
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
    FileData { file_data: FileData },
    FunctionCall { function_call: FunctionCall },
    FunctionResponse { function_response: FunctionResponse },
}

/// Inline data (base64)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

/// File data (GCS URI)
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

/// Content with role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

/// Candidate response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub content: Content,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation_metadata: Option<CitationMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groundingattributions: Option<Vec<GroundingAttribution>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs_result: Option<LogprobsResult>,
}

/// Finish reasons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason {
    #[serde(rename = "FINISH_REASON_UNSPECIFIED")]
    Unspecified,
    #[serde(rename = "STOP")]
    Stop,
    #[serde(rename = "MAX_TOKENS")]
    MaxTokens,
    #[serde(rename = "SAFETY")]
    Safety,
    #[serde(rename = "RECITATION")]
    Recitation,
    #[serde(rename = "OTHER")]
    Other,
}

/// Safety rating
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: SafetyCategory,
    pub probability: SafetyProbability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
}

/// Safety probability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyProbability {
    #[serde(rename = "NEGLIGIBLE")]
    Negligible,
    #[serde(rename = "LOW")]
    Low,
    #[serde(rename = "MEDIUM")]
    Medium,
    #[serde(rename = "HIGH")]
    High,
}

/// Citation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationMetadata {
    pub citation_sources: Vec<CitationSource>,
}

/// Citation source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// Grounding attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingAttribution {
    pub content: Content,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

/// Logprobs result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogprobsResult {
    pub top_candidates: Vec<TopCandidate>,
    pub chosen_candidates: Vec<Candidate>,
}

/// Top candidate for logprobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopCandidate {
    pub token: String,
    pub log_probability: f32,
}

/// Usage metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetadata {
    pub prompt_token_count: i32,
    pub candidates_token_count: i32,
    pub total_token_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content_token_count: Option<i32>,
}

/// Generate content response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateContentResponse {
    pub candidates: Vec<Candidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_feedback: Option<PromptFeedback>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
}

/// Prompt feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFeedback {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_reason: Option<BlockReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

/// Block reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlockReason {
    #[serde(rename = "BLOCKED_REASON_UNSPECIFIED")]
    Unspecified,
    #[serde(rename = "SAFETY")]
    Safety,
    #[serde(rename = "OTHER")]
    Other,
}

/// Gemini cost calculator
pub struct GeminiCostCalculator;

impl GeminiCostCalculator {
    /// Calculate cost for Gemini models
    pub fn calculate_cost(
        model: &str,
        input_tokens: usize,
        output_tokens: usize,
    ) -> Result<f64, crate::ProviderError> {
        let input_tokens = u32::try_from(input_tokens).map_err(|_| {
            crate::ProviderError::invalid_request("vertex_ai", "input token count exceeds u32")
        })?;
        let output_tokens = u32::try_from(output_tokens).map_err(|_| {
            crate::ProviderError::invalid_request("vertex_ai", "output token count exceeds u32")
        })?;
        super::calculate_vertex_cost(model, input_tokens, output_tokens)
    }
}

#[cfg(test)]
mod cost_tests {
    use super::GeminiCostCalculator;
    use crate::ProviderError;

    #[test]
    fn calculator_uses_vertex_authority_and_rejects_unknown_models() {
        let cost = GeminiCostCalculator::calculate_cost("gemini-1.5-pro", 1_000, 500)
            .expect("catalogued Vertex model should be priced");
        assert!((cost - 0.00875).abs() < 1e-12);

        let unknown = GeminiCostCalculator::calculate_cost("unknown-google-model", 1_000, 500);
        assert!(matches!(unknown, Err(ProviderError::ModelNotFound { .. })));
    }
}
