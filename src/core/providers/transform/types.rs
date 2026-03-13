//! Request/Response Transformation Engine - Format normalization across providers
//!
//! This module implements transformation pipelines that convert between
//! OpenAI-compatible format and provider-specific formats.
//!
//! **Internal pipeline DTOs** — Types here are serde adaptation layers for
//! provider-specific JSON schemas. They are intentionally separate from
//! `core::types` (the canonical API types) to isolate provider format changes
//! from the public API surface. Do not use outside the `transform` module.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::core::providers::{ProviderType, unified_provider::ProviderError};

/// Result type for provider operations
pub type ProviderResult<T> = Result<T, ProviderError>;

/// Generic request types for different endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stream: Option<bool>,
    pub functions: Option<Vec<Function>>,
    pub function_call: Option<FunctionCall>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub top_p: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub stop: Option<Vec<String>>,
    pub response_format: Option<ResponseFormat>,
    pub seed: Option<i32>,
    pub logit_bias: Option<HashMap<String, f64>>,
    pub user: Option<String>,
    pub extra_headers: Option<HashMap<String, String>>,
    pub extra_body: Option<Map<String, Value>>,
    pub thinking: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<Value>, // Can be string or structured content
    pub name: Option<String>,
    pub function_call: Option<FunctionCallResponse>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FunctionCall {
    Auto,
    None,
    Specific { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Specific {
        #[serde(rename = "type")]
        tool_type: String,
        function: ToolFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCallResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallResponse {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String, // "text" or "json_object"
}

/// Generic response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
    pub system_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
    pub logprobs: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Embedding request/response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
    pub encoding_format: Option<String>,
    pub dimensions: Option<u32>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    String(String),
    Strings(Vec<String>),
    Tokens(Vec<u32>),
    TokenArrays(Vec<Vec<u32>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f64>,
    pub index: u32,
}

/// Generic provider request/response (what gets sent to actual provider APIs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub endpoint: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
    pub query_params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Value,
    pub latency_ms: f64,
}

/// Transform result with metadata
#[derive(Debug, Clone)]
pub struct TransformResult<T> {
    pub data: T,
    pub metadata: TransformMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformMetadata {
    pub provider_type: ProviderType,
    pub original_model: String,
    pub transformed_model: String,
    pub transformations_applied: Vec<String>,
    pub warnings: Vec<String>,
    pub cost_estimate: Option<f64>,
}

/// Transformation engine trait
#[async_trait]
pub trait TransformEngine: Send + Sync {
    /// Transform OpenAI format request to provider-specific format
    async fn transform_chat_request(
        &self,
        request: &TransformChatRequest,
        provider_type: &ProviderType,
        provider_config: &HashMap<String, Value>,
    ) -> ProviderResult<TransformResult<ProviderRequest>>;

    /// Transform provider response back to OpenAI format
    async fn transform_chat_response(
        &self,
        response: &ProviderResponse,
        provider_type: &ProviderType,
        original_request: &TransformChatRequest,
    ) -> ProviderResult<TransformResult<ChatResponse>>;

    /// Transform embedding request
    async fn transform_embedding_request(
        &self,
        request: &EmbeddingRequest,
        provider_type: &ProviderType,
        provider_config: &HashMap<String, Value>,
    ) -> ProviderResult<TransformResult<ProviderRequest>>;

    /// Transform embedding response
    async fn transform_embedding_response(
        &self,
        response: &ProviderResponse,
        provider_type: &ProviderType,
        original_request: &EmbeddingRequest,
    ) -> ProviderResult<TransformResult<EmbeddingResponse>>;

    /// Get supported transformations for a provider
    fn get_supported_transformations(&self, provider_type: &ProviderType) -> Vec<String>;

    /// Validate request compatibility with provider
    async fn validate_request_compatibility(
        &self,
        request: &TransformChatRequest,
        provider_type: &ProviderType,
    ) -> ProviderResult<Vec<String>>;
}

/// Transform pipeline for chaining transformations
pub struct TransformPipeline {
    pub(crate) transforms: Vec<Box<dyn Transform>>,
}

/// Individual transformation step
#[async_trait]
pub trait Transform: Send + Sync {
    /// Apply transformation to request
    async fn transform_request(
        &self,
        request: Value,
        context: &TransformContext,
    ) -> ProviderResult<Value>;

    /// Apply reverse transformation to response
    async fn transform_response(
        &self,
        response: Value,
        context: &TransformContext,
    ) -> ProviderResult<Value>;

    /// Get transformation name
    fn name(&self) -> &str;
}

/// Context for transformations
#[derive(Debug, Clone)]
pub struct TransformContext {
    pub provider_type: ProviderType,
    pub original_model: String,
    pub target_model: String,
    pub config: HashMap<String, Value>,
    pub metadata: HashMap<String, String>,
}
