//! OpenAI Responses API types
//!
//! This module defines request and response types for the OpenAI Responses API
//! (POST /v1/responses), which is the new standard replacing the Assistants API.
//!
//! Reference: <https://platform.openai.com/docs/api-reference/responses>

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Request types ────────────────────────────────────────────────────────────

/// POST /v1/responses request body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiRequest {
    /// Model to use (e.g. "gpt-4o", "gpt-4.1")
    pub model: String,

    /// Input items: a plain string or an array of input objects
    pub input: ResponseInput,

    /// System-level instructions (equivalent to a system message)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// ID of a previous response to continue from (stateful chaining)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,

    /// Whether to store this response for future chaining (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,

    /// Built-in or function tools available to the model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ResponseTool>>,

    /// Stream the response as server-sent events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Run as a background (async) task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,

    /// Maximum number of output tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,

    /// Sampling temperature (0.0–2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling probability (0.0–1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Stable user identifier for abuse detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Reasoning configuration (for o-series / extended-thinking models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningParams>,

    /// Metadata key-value pairs (max 16 pairs, 64-char keys, 512-char values)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,

    /// How to truncate context when the model's context window is exceeded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<String>,
}

/// Input to the Responses API — a raw string or a list of structured items
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInput {
    /// Simple text prompt
    Text(String),
    /// Array of structured input items
    Items(Vec<ResponseInputItem>),
}

/// A single item in the `input` array
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputItem {
    /// A conversational message (user / assistant / system)
    Message(ResponseInputMessage),
}

/// A conversational message inside `input`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseInputMessage {
    /// Role: "user" | "assistant" | "system"
    pub role: String,
    /// Content: plain string or array of content parts
    pub content: ResponseInputContent,
}

/// Message content — plain text or multi-part
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInputContent {
    /// Single text string
    Text(String),
    /// Array of typed content parts
    Parts(Vec<ResponseInputContentPart>),
}

/// A typed content part inside a Responses API message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseInputContentPart {
    /// Plain text input
    InputText { text: String },
    /// Base-64-encoded or URL-referenced image
    InputImage {
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// Plain text output (used in assistant turns returned by the API)
    OutputText { text: String },
}

// ── Tool types ────────────────────────────────────────────────────────────────

/// A tool definition for the Responses API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseTool {
    /// Web-search built-in tool
    WebSearch(WebSearchTool),
    /// Alias used in some preview builds
    #[serde(rename = "web_search_preview")]
    WebSearchPreview(WebSearchTool),
    /// File-search built-in tool
    FileSearch(FileSearchTool),
    /// Code-interpreter built-in tool
    CodeInterpreter(CodeInterpreterTool),
    /// Computer-use built-in tool (preview)
    #[serde(rename = "computer_use_preview")]
    ComputerUsePreview(ComputerUseTool),
    /// MCP server integration
    Mcp(McpTool),
    /// Regular function-calling tool
    Function(ResponseFunctionTool),
}

/// Web-search tool configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSearchTool {
    /// Optional user location for localised results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<Value>,
    /// Search context size: "low" | "medium" | "high"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<String>,
}

/// File-search tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchTool {
    /// Vector store IDs to search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_store_ids: Option<Vec<String>>,
    /// Maximum number of chunks to retrieve
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_num_results: Option<u32>,
    /// Ranking options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_options: Option<Value>,
    /// Filters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<Value>,
}

/// Code-interpreter tool configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeInterpreterTool {
    /// Container configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<Value>,
}

/// Computer-use tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerUseTool {
    /// Display width in pixels
    pub display_width: u32,
    /// Display height in pixels
    pub display_height: u32,
    /// Environment type: "browser" | "desktop"
    pub environment: String,
}

/// MCP server tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Human-readable server label
    pub server_label: String,
    /// Base URL of the MCP server
    pub server_url: String,
    /// Allowed tool names (empty = all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    /// Additional headers for MCP server requests
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::HashMap<String, String>>,
    /// Whether to require human approval before tool execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_approval: Option<Value>,
}

/// Regular function-calling tool (same schema as chat completions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunctionTool {
    /// Function definition
    pub function: ResponseFunctionDefinition,
}

/// Function definition for the Responses API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunctionDefinition {
    /// Function name
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the function parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Value>,
    /// Whether to enforce strict schema adherence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Reasoning / extended-thinking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningParams {
    /// Effort level: "low" | "medium" | "high"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Whether to surface the reasoning summary in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

// ── Response types ────────────────────────────────────────────────────────────

/// POST /v1/responses response object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiResponse {
    /// Unique identifier (e.g. "resp_…")
    pub id: String,
    /// Always "response"
    pub object: String,
    /// Unix timestamp of creation
    pub created_at: i64,
    /// "completed" | "in_progress" | "failed" | "incomplete"
    pub status: String,
    /// Model that was used
    pub model: String,
    /// Output items produced by the model
    pub output: Vec<ResponseOutputItem>,
    /// Token usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponseUsage>,
    /// Error details if `status` is "failed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseApiError>,
    /// Parallel response ID (for background runs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    /// Metadata echoed from the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// An item in the `output` array
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseOutputItem {
    /// Assistant message containing text or refusal
    Message(ResponseOutputMessage),
    /// A function call invocation
    FunctionCall(ResponseFunctionCall),
    /// Result of a built-in tool call
    WebSearchCall(ResponseToolCall),
    /// Result of a file-search call
    FileSearchCall(ResponseToolCall),
    /// Result of a code-interpreter call
    CodeInterpreterCall(ResponseToolCall),
    /// Computer-use action call
    ComputerCall(ResponseToolCall),
    /// MCP tool call
    McpCall(ResponseToolCall),
    /// Extended reasoning / thinking block
    Reasoning(ResponseReasoningItem),
}

/// An assistant message in the output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseOutputMessage {
    /// Unique item ID
    pub id: String,
    /// Always "assistant"
    pub role: String,
    /// "completed" | "in_progress" | "incomplete"
    pub status: String,
    /// Content parts (text, refusal, …)
    pub content: Vec<ResponseOutputContent>,
}

/// A content part inside an output message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseOutputContent {
    /// Model-generated text
    OutputText {
        /// The text string
        text: String,
        /// Annotation objects (citations, links, …)
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<Vec<Value>>,
        /// Log-probability information
        #[serde(skip_serializing_if = "Option::is_none")]
        logprobs: Option<Vec<Value>>,
    },
    /// Model refused to answer
    Refusal {
        /// The refusal message
        refusal: String,
    },
}

/// A function call in the output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFunctionCall {
    /// Unique call ID
    pub id: String,
    /// Function name
    pub name: String,
    /// JSON-encoded arguments
    pub arguments: String,
    /// "in_progress" | "completed" | "incomplete"
    pub status: String,
    /// Call ID used to correlate with tool outputs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
}

/// Generic built-in tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseToolCall {
    /// Unique call ID
    pub id: String,
    /// "in_progress" | "completed" | "incomplete" | "searching" | "failed"
    pub status: String,
    /// Tool-specific result data
    #[serde(flatten)]
    pub data: Value,
}

/// Extended reasoning / thinking block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseReasoningItem {
    /// Unique item ID
    pub id: String,
    /// "completed"
    pub status: String,
    /// Summary of the reasoning (if enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Vec<Value>>,
}

/// Token usage for a Responses API response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseUsage {
    /// Tokens in the input
    pub input_tokens: u32,
    /// Tokens in the output
    pub output_tokens: u32,
    /// Total tokens consumed
    pub total_tokens: u32,
    /// Detailed breakdown (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens_details: Option<ResponseInputTokensDetails>,
    /// Output tokens breakdown (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens_details: Option<ResponseOutputTokensDetails>,
}

/// Input-token breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseInputTokensDetails {
    /// Tokens retrieved from the context cache
    pub cached_tokens: u32,
}

/// Output-token breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseOutputTokensDetails {
    /// Tokens used for extended reasoning
    pub reasoning_tokens: u32,
}

/// API error embedded in a failed response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseApiError {
    /// OpenAI error code
    pub code: String,
    /// Human-readable message
    pub message: String,
}

// ── Streaming event types ─────────────────────────────────────────────────────

/// A server-sent event emitted during Responses API streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseStreamEvent {
    /// Initial event — response shell created
    #[serde(rename = "response.created")]
    ResponseCreated { response: Box<ResponsesApiResponse> },
    /// Model started producing output
    #[serde(rename = "response.in_progress")]
    ResponseInProgress { response: Box<ResponsesApiResponse> },
    /// A new output item was added to `output`
    #[serde(rename = "response.output_item.added")]
    ResponseOutputItemAdded {
        output_index: u32,
        item: ResponseOutputItem,
    },
    /// A new content part started streaming
    #[serde(rename = "response.content_part.added")]
    ResponseContentPartAdded {
        output_index: u32,
        content_index: u32,
        part: ResponseOutputContent,
    },
    /// Incremental text delta
    #[serde(rename = "response.output_text.delta")]
    ResponseOutputTextDelta {
        output_index: u32,
        content_index: u32,
        delta: String,
    },
    /// Text part finished streaming
    #[serde(rename = "response.output_text.done")]
    ResponseOutputTextDone {
        output_index: u32,
        content_index: u32,
        text: String,
    },
    /// A content part finished
    #[serde(rename = "response.content_part.done")]
    ResponseContentPartDone {
        output_index: u32,
        content_index: u32,
        part: ResponseOutputContent,
    },
    /// An output item finished
    #[serde(rename = "response.output_item.done")]
    ResponseOutputItemDone {
        output_index: u32,
        item: ResponseOutputItem,
    },
    /// Entire response completed
    #[serde(rename = "response.completed")]
    ResponseCompleted { response: Box<ResponsesApiResponse> },
    /// Response failed
    #[serde(rename = "response.failed")]
    ResponseFailed { response: Box<ResponsesApiResponse> },
    /// Incremental function-call arguments
    #[serde(rename = "response.function_call_arguments.delta")]
    ResponseFunctionCallArgumentsDelta {
        output_index: u32,
        call_id: String,
        delta: String,
    },
    /// Function-call arguments finished
    #[serde(rename = "response.function_call_arguments.done")]
    ResponseFunctionCallArgumentsDone {
        output_index: u32,
        call_id: String,
        arguments: String,
    },
    /// Catch-all for forward-compatible events
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_text_input_deserialise() {
        let json = r#"{"model":"gpt-4o","input":"Hello"}"#;
        let req: ResponsesApiRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert!(matches!(req.input, ResponseInput::Text(_)));
    }

    #[test]
    fn test_request_array_input_deserialise() {
        let json = r#"{
            "model": "gpt-4o",
            "input": [
                {"type": "message", "role": "user", "content": "Hello"}
            ]
        }"#;
        let req: ResponsesApiRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.input, ResponseInput::Items(_)));
    }

    #[test]
    fn test_request_with_instructions() {
        let json = r#"{"model":"gpt-4o","input":"Hi","instructions":"Be concise"}"#;
        let req: ResponsesApiRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.instructions.as_deref(), Some("Be concise"));
    }

    #[test]
    fn test_request_with_previous_response_id() {
        let json = r#"{"model":"gpt-4o","input":"Follow up","previous_response_id":"resp_abc"}"#;
        let req: ResponsesApiRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.previous_response_id.as_deref(), Some("resp_abc"));
    }

    #[test]
    fn test_request_with_store_flag() {
        let json = r#"{"model":"gpt-4o","input":"Hi","store":true}"#;
        let req: ResponsesApiRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.store, Some(true));
    }

    #[test]
    fn test_web_search_tool_deserialise() {
        let json = r#"{"type":"web_search"}"#;
        let tool: ResponseTool = serde_json::from_str(json).unwrap();
        assert!(matches!(tool, ResponseTool::WebSearch(_)));
    }

    #[test]
    fn test_file_search_tool_deserialise() {
        let json = r#"{"type":"file_search","vector_store_ids":["vs_abc"]}"#;
        let tool: ResponseTool = serde_json::from_str(json).unwrap();
        assert!(matches!(tool, ResponseTool::FileSearch(_)));
    }

    #[test]
    fn test_mcp_tool_deserialise() {
        let json = r#"{
            "type": "mcp",
            "server_label": "my-server",
            "server_url": "https://example.com/mcp"
        }"#;
        let tool: ResponseTool = serde_json::from_str(json).unwrap();
        assert!(matches!(tool, ResponseTool::Mcp(_)));
    }

    #[test]
    fn test_function_tool_deserialise() {
        let json = r#"{
            "type": "function",
            "function": {"name": "get_weather", "description": "Get weather"}
        }"#;
        let tool: ResponseTool = serde_json::from_str(json).unwrap();
        assert!(matches!(tool, ResponseTool::Function(_)));
    }

    #[test]
    fn test_response_serialise() {
        let resp = ResponsesApiResponse {
            id: "resp_123".to_string(),
            object: "response".to_string(),
            created_at: 1_700_000_000,
            status: "completed".to_string(),
            model: "gpt-4o".to_string(),
            output: vec![],
            usage: None,
            error: None,
            previous_response_id: None,
            metadata: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"object\":\"response\""));
        assert!(json.contains("resp_123"));
    }

    #[test]
    fn test_response_usage_defaults() {
        let usage = ResponseUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_stream_event_text_delta_serialise() {
        let event = ResponseStreamEvent::ResponseOutputTextDelta {
            output_index: 0,
            content_index: 0,
            delta: "Hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("response.output_text.delta"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_reasoning_params_deserialise() {
        let json = r#"{"effort":"high","summary":"auto"}"#;
        let params: ReasoningParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.effort.as_deref(), Some("high"));
    }
}
