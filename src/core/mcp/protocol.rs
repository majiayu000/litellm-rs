//! MCP Protocol Types
//!
//! JSON-RPC 2.0 message types for MCP communication.

use super::error::McpError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 version constant
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,

    /// Request method
    pub method: String,

    /// Request parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,

    /// Request ID (for matching responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
}

impl JsonRpcRequest {
    /// Create a new JSON-RPC request
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id: Some(Value::Number(1.into())),
        }
    }

    /// Create a request with a specific ID
    pub fn with_id(mut self, id: impl Into<Value>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Create a notification (no ID, no response expected)
    pub fn notification(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id: None,
        }
    }

    /// Check if this is a notification
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,

    /// Response result (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    /// Response error (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,

    /// Request ID this is responding to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(result: Value, id: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id: Some(id),
        }
    }

    /// Create an error response
    pub fn error(error: JsonRpcError, id: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }

    /// Check if this response is an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Check if this response is successful
    pub fn is_success(&self) -> bool {
        self.result.is_some()
    }
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,

    /// Error message
    pub message: String,

    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    /// Create a new error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create an error with additional data
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Parse error (-32700)
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    /// Invalid request (-32600)
    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }

    /// Method not found (-32601)
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found")
    }

    /// Invalid params (-32602)
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }

    /// Internal error (-32603)
    pub fn internal_error() -> Self {
        Self::new(-32603, "Internal error")
    }

    /// Server error (-32000 to -32099)
    pub fn server_error(code: i32, message: impl Into<String>) -> Self {
        let code = code.clamp(-32099, -32000);
        Self::new(code, message)
    }

    /// Build protocol error payload from typed MCP errors.
    pub fn from_mcp_error(error: &McpError) -> Self {
        use crate::utils::error::canonical::CanonicalError;

        let code = match error {
            McpError::ServerNotFound { .. } | McpError::ToolNotFound { .. } => -32001,
            McpError::AuthenticationError { .. } | McpError::AuthorizationError { .. } => -32004,
            McpError::RateLimitExceeded { .. } => -32029,
            McpError::Timeout { .. } => -32008,
            McpError::ConnectionError { .. } | McpError::TransportError { .. } => -32010,
            McpError::InvalidUrl { .. } | McpError::ProtocolError { .. } => -32600,
            McpError::ConfigurationError { .. } => -32602,
            McpError::ToolExecutionError { .. } | McpError::SerializationError { .. } => -32603,
            McpError::ServerAlreadyExists { .. } => -32009,
            McpError::ToolDefinitionChanged { .. } => -32009,
            McpError::ValidationError { .. } => -32602,
        };

        let mut rpc_error = Self::new(code, error.to_string());
        rpc_error.data = Some(serde_json::json!({
            "canonical_code": error.canonical_code().as_str(),
            "retryable": error.canonical_retryable(),
        }));
        rpc_error
    }
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

/// MCP-specific message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpMessage {
    /// JSON-RPC request
    Request(JsonRpcRequest),

    /// JSON-RPC response
    Response(JsonRpcResponse),

    /// Batch of requests/responses
    Batch(Vec<McpMessage>),
}

impl McpMessage {
    /// Try to parse from JSON string
    pub fn parse(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// MCP Method names as constants
pub mod methods {
    /// Initialize the connection
    pub const INITIALIZE: &str = "initialize";

    /// List available tools
    pub const LIST_TOOLS: &str = "tools/list";

    /// Call a tool
    pub const CALL_TOOL: &str = "tools/call";

    /// List available resources
    pub const LIST_RESOURCES: &str = "resources/list";

    /// Read a resource
    pub const READ_RESOURCE: &str = "resources/read";

    /// List available prompts
    pub const LIST_PROMPTS: &str = "prompts/list";

    /// Get a prompt
    pub const GET_PROMPT: &str = "prompts/get";

    /// Complete (text completion)
    pub const COMPLETE: &str = "completion/complete";

    /// Set logging level
    pub const SET_LOGGING_LEVEL: &str = "logging/setLevel";

    /// Ping (health check)
    pub const PING: &str = "ping";
}

/// MCP Capability types
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpCapabilities {
    /// Supported tools capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,

    /// Supported resources capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,

    /// Supported prompts capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,

    /// Supported logging capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapability>,
}

/// Tools capability
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsCapability {
    /// Whether list_changed notifications are supported
    #[serde(default)]
    pub list_changed: bool,
}

/// Resources capability
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourcesCapability {
    /// Whether subscribe is supported
    #[serde(default)]
    pub subscribe: bool,

    /// Whether list_changed notifications are supported
    #[serde(default)]
    pub list_changed: bool,
}

/// Prompts capability
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptsCapability {
    /// Whether list_changed notifications are supported
    #[serde(default)]
    pub list_changed: bool,
}

/// Logging capability
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoggingCapability {}

/// Initialize request params
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    /// Protocol version
    pub protocol_version: String,

    /// Client capabilities
    pub capabilities: McpCapabilities,

    /// Client info
    pub client_info: ClientInfo,
}

/// Client info for initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client name
    pub name: String,

    /// Client version
    pub version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: "litellm-rs".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_request_new() {
        let req = JsonRpcRequest::new("test_method", Some(serde_json::json!({"key": "value"})));
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "test_method");
        assert!(req.params.is_some());
        assert!(req.id.is_some());
    }

    #[test]
    fn test_jsonrpc_request_notification() {
        let req = JsonRpcRequest::notification("test_method", None);
        assert!(req.is_notification());
        assert!(req.id.is_none());
    }

    #[test]
    fn test_jsonrpc_response_success() {
        let resp =
            JsonRpcResponse::success(serde_json::json!({"result": "ok"}), Value::Number(1.into()));
        assert!(resp.is_success());
        assert!(!resp.is_error());
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let resp = JsonRpcResponse::error(
            JsonRpcError::method_not_found(),
            Some(Value::Number(1.into())),
        );
        assert!(resp.is_error());
        assert!(!resp.is_success());
    }

    #[test]
    fn test_jsonrpc_error_codes() {
        assert_eq!(JsonRpcError::parse_error().code, -32700);
        assert_eq!(JsonRpcError::invalid_request().code, -32600);
        assert_eq!(JsonRpcError::method_not_found().code, -32601);
        assert_eq!(JsonRpcError::invalid_params().code, -32602);
        assert_eq!(JsonRpcError::internal_error().code, -32603);
    }

    #[test]
    fn test_jsonrpc_error_server_error_clamping() {
        let err = JsonRpcError::server_error(-99999, "test");
        assert!(err.code >= -32099 && err.code <= -32000);
    }

    #[test]
    fn test_jsonrpc_error_from_mcp_error_includes_canonical_data() {
        let error = McpError::RateLimitExceeded {
            server_name: "github".to_string(),
            retry_after_ms: Some(1000),
        };

        let rpc_error = JsonRpcError::from_mcp_error(&error);
        assert_eq!(rpc_error.code, -32029);
        let data = rpc_error.data.expect("canonical data should exist");
        assert_eq!(data["canonical_code"], "RATE_LIMITED");
        assert_eq!(data["retryable"], true);
    }

    #[test]
    fn test_jsonrpc_error_display() {
        let err = JsonRpcError::method_not_found();
        assert!(err.to_string().contains("-32601"));
        assert!(err.to_string().contains("Method not found"));
    }

    #[test]
    fn test_mcp_message_parse() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let msg = McpMessage::parse(json).unwrap();
        match msg {
            McpMessage::Request(req) => {
                assert_eq!(req.method, "test");
            }
            _ => panic!("Expected request"),
        }
    }

    #[test]
    fn test_request_serialization() {
        let req = JsonRpcRequest::new("tools/list", None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("tools/list"));
        assert!(json.contains("2.0"));
    }

    #[test]
    fn test_client_info_default() {
        let info = ClientInfo::default();
        assert_eq!(info.name, "litellm-rs");
        assert!(!info.version.is_empty());
    }

    #[test]
    fn test_capabilities_default() {
        let caps = McpCapabilities::default();
        assert!(caps.tools.is_none());
        assert!(caps.resources.is_none());
    }

    #[test]
    fn test_method_constants() {
        assert_eq!(methods::INITIALIZE, "initialize");
        assert_eq!(methods::LIST_TOOLS, "tools/list");
        assert_eq!(methods::CALL_TOOL, "tools/call");
    }
}
