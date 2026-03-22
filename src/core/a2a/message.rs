//! A2A Message Types
//!
//! JSON-RPC 2.0 message types for A2A protocol communication.

use std::sync::atomic::{AtomicU64, Ordering};

use super::error::A2AError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Monotonically increasing counter for unique JSON-RPC request IDs.
static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

/// Return the next unique JSON-RPC request ID.
fn next_id() -> Value {
    Value::Number(REQUEST_ID.fetch_add(1, Ordering::Relaxed).into())
}

/// JSON-RPC 2.0 version constant
pub const JSONRPC_VERSION: &str = "2.0";

/// A2A Message (JSON-RPC 2.0 request)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    /// JSON-RPC version (must be "2.0")
    pub jsonrpc: String,

    /// Request method
    pub method: String,

    /// Request parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<MessageParams>,

    /// Request ID
    pub id: Value,
}

impl A2AMessage {
    /// Create a new send message request
    pub fn send(message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "message/send".to_string(),
            params: Some(MessageParams {
                message: Message {
                    role: "user".to_string(),
                    parts: vec![MessagePart::Text {
                        text: message.into(),
                    }],
                },
                configuration: None,
            }),
            id: next_id(),
        }
    }

    /// Create a task status request
    pub fn get_task(task_id: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "tasks/get".to_string(),
            params: Some(MessageParams {
                message: Message {
                    role: "system".to_string(),
                    parts: vec![MessagePart::Text {
                        text: task_id.into(),
                    }],
                },
                configuration: None,
            }),
            id: next_id(),
        }
    }

    /// Create a cancel task request
    pub fn cancel_task(task_id: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: "tasks/cancel".to_string(),
            params: Some(MessageParams {
                message: Message {
                    role: "system".to_string(),
                    parts: vec![MessagePart::Text {
                        text: task_id.into(),
                    }],
                },
                configuration: None,
            }),
            id: next_id(),
        }
    }

    /// Set request ID
    pub fn with_id(mut self, id: impl Into<Value>) -> Self {
        self.id = id.into();
        self
    }

    /// Add configuration
    pub fn with_config(mut self, config: MessageConfiguration) -> Self {
        if let Some(ref mut params) = self.params {
            params.configuration = Some(config);
        }
        self
    }
}

/// Message parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageParams {
    /// The message to send
    pub message: Message,

    /// Optional configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<MessageConfiguration>,
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role (user, assistant, system)
    pub role: String,

    /// Message content parts
    pub parts: Vec<MessagePart>,
}

impl Message {
    /// Create a user message with text
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            parts: vec![MessagePart::Text { text: text.into() }],
        }
    }

    /// Create an assistant message with text
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            parts: vec![MessagePart::Text { text: text.into() }],
        }
    }

    /// Create a system message with text
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            parts: vec![MessagePart::Text { text: text.into() }],
        }
    }
}

/// Message content part
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MessagePart {
    /// Text content
    Text { text: String },

    /// File/data content
    File {
        #[serde(rename = "mimeType")]
        mime_type: String,
        data: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },

    /// Tool use request
    ToolUse {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        name: String,
        input: Value,
    },

    /// Tool result
    ToolResult {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        content: String,
        #[serde(rename = "isError", default)]
        is_error: bool,
    },
}

impl MessagePart {
    /// Create text part
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create file part
    pub fn file(mime_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::File {
            mime_type: mime_type.into(),
            data: data.into(),
            name: None,
        }
    }

    /// Create tool use part
    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        Self::ToolUse {
            tool_use_id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create tool result part
    pub fn tool_result(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: id.into(),
            content: content.into(),
            is_error: false,
        }
    }
}

/// Message configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageConfiguration {
    /// Whether to wait for completion
    #[serde(
        rename = "acceptedOutputModes",
        skip_serializing_if = "Option::is_none"
    )]
    pub accepted_output_modes: Option<Vec<String>>,

    /// Enable streaming
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,

    /// Push notification URL
    #[serde(
        rename = "pushNotificationConfig",
        skip_serializing_if = "Option::is_none"
    )]
    pub push_notification_config: Option<PushNotificationConfig>,

    /// History behavior
    #[serde(rename = "historyLength", skip_serializing_if = "Option::is_none")]
    pub history_length: Option<u32>,
}

/// Push notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotificationConfig {
    /// Callback URL
    pub url: String,

    /// Authentication token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// A2A Response (JSON-RPC 2.0 response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AResponse {
    /// JSON-RPC version
    pub jsonrpc: String,

    /// Response result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResult>,

    /// Response error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<A2AResponseError>,

    /// Request ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
}

impl A2AResponse {
    /// Check if response is successful
    pub fn is_success(&self) -> bool {
        self.result.is_some() && self.error.is_none()
    }

    /// Check if response is an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Get the task result
    pub fn get_result(&self) -> Option<&TaskResult> {
        self.result.as_ref()
    }

    /// Get the error
    pub fn get_error(&self) -> Option<&A2AResponseError> {
        self.error.as_ref()
    }
}

/// Task result from A2A response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID
    pub id: String,

    /// Task state
    pub status: TaskStatus,

    /// Task artifacts (outputs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,

    /// History of messages
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<Message>,
}

/// Task status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    /// Current state
    pub state: TaskState,

    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,

    /// Timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
}

/// Task state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    /// Task is pending
    #[default]
    Pending,

    /// Task is running
    Running,

    /// Task requires input
    #[serde(rename = "input-required")]
    InputRequired,

    /// Task completed successfully
    Completed,

    /// Task failed
    Failed,

    /// Task was cancelled
    Cancelled,
}

impl TaskState {
    /// Check if task is complete (finished in any state)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Check if task is still running
    pub fn is_running(&self) -> bool {
        matches!(self, TaskState::Running)
    }

    /// Check if task succeeded
    pub fn is_success(&self) -> bool {
        matches!(self, TaskState::Completed)
    }
}

/// Task artifact (output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Artifact name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Artifact content parts
    pub parts: Vec<MessagePart>,

    /// Index in artifact list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

/// A2A error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AResponseError {
    /// Error code
    pub code: i32,

    /// Error message
    pub message: String,

    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl A2AResponseError {
    /// Create a new error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
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

    /// Task not found (-32001)
    pub fn task_not_found() -> Self {
        Self::new(-32001, "Task not found")
    }

    /// Task cancelled (-32002)
    pub fn task_cancelled() -> Self {
        Self::new(-32002, "Task cancelled")
    }

    /// Build protocol error payload from a typed A2A error.
    pub fn from_a2a_error(error: &A2AError) -> Self {
        use crate::utils::error::canonical::CanonicalError;

        let code = match error {
            A2AError::AgentNotFound { .. } | A2AError::TaskNotFound { .. } => -32001,
            A2AError::AgentBusy { .. } => -32002,
            A2AError::AgentAlreadyExists { .. } => -32003,
            A2AError::AuthenticationError { .. } => -32004,
            A2AError::RateLimitExceeded { .. } => -32029,
            A2AError::Timeout { .. } => -32008,
            A2AError::ConnectionError { .. } => -32010,
            A2AError::ProtocolError { .. } | A2AError::InvalidRequest { .. } => -32600,
            A2AError::UnsupportedProvider { .. } => -32601,
            A2AError::ContentBlocked { .. } => -32602,
            A2AError::TaskFailed { .. }
            | A2AError::ConfigurationError { .. }
            | A2AError::SerializationError { .. } => -32603,
        };

        let mut response = Self::new(code, error.to_string());
        response.data = Some(serde_json::json!({
            "canonical_code": error.canonical_code().as_str(),
            "retryable": error.canonical_retryable(),
        }));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a2a_message_send() {
        let msg = A2AMessage::send("Hello, agent!");
        assert_eq!(msg.method, "message/send");
        assert!(msg.params.is_some());
    }

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello");
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.parts.len(), 1);

        let asst_msg = Message::assistant("Hi there");
        assert_eq!(asst_msg.role, "assistant");
    }

    #[test]
    fn test_message_part_text() {
        let part = MessagePart::text("Hello");
        match part {
            MessagePart::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected text part"),
        }
    }

    #[test]
    fn test_message_part_file() {
        let part = MessagePart::file("image/png", "base64data");
        match part {
            MessagePart::File {
                mime_type, data, ..
            } => {
                assert_eq!(mime_type, "image/png");
                assert_eq!(data, "base64data");
            }
            _ => panic!("Expected file part"),
        }
    }

    #[test]
    fn test_message_part_tool_use() {
        let part = MessagePart::tool_use("id-1", "search", serde_json::json!({"query": "test"}));
        match part {
            MessagePart::ToolUse {
                tool_use_id,
                name,
                input,
            } => {
                assert_eq!(tool_use_id, "id-1");
                assert_eq!(name, "search");
                assert_eq!(input["query"], "test");
            }
            _ => panic!("Expected tool use part"),
        }
    }

    #[test]
    fn test_task_state_terminal() {
        assert!(TaskState::Completed.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
        assert!(!TaskState::Running.is_terminal());
        assert!(!TaskState::Pending.is_terminal());
    }

    #[test]
    fn test_task_state_success() {
        assert!(TaskState::Completed.is_success());
        assert!(!TaskState::Failed.is_success());
    }

    #[test]
    fn test_a2a_response_error_codes() {
        assert_eq!(A2AResponseError::parse_error().code, -32700);
        assert_eq!(A2AResponseError::invalid_request().code, -32600);
        assert_eq!(A2AResponseError::method_not_found().code, -32601);
        assert_eq!(A2AResponseError::task_not_found().code, -32001);
    }

    #[test]
    fn test_a2a_response_error_from_a2a_error_includes_canonical_data() {
        let error = A2AError::RateLimitExceeded {
            agent_name: "agent-a".to_string(),
            retry_after_ms: Some(500),
        };

        let response_error = A2AResponseError::from_a2a_error(&error);
        assert_eq!(response_error.code, -32029);

        let data = response_error.data.expect("canonical data should exist");
        assert_eq!(data["canonical_code"], "RATE_LIMITED");
        assert_eq!(data["retryable"], true);
    }

    #[test]
    fn test_message_serialization() {
        let msg = A2AMessage::send("Test message");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("message/send"));
        assert!(json.contains("Test message"));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "jsonrpc": "2.0",
            "result": {
                "id": "task-123",
                "status": {
                    "state": "completed"
                },
                "artifacts": []
            },
            "id": 1
        }"#;

        let response: A2AResponse = serde_json::from_str(json).unwrap();
        assert!(response.is_success());
        assert_eq!(response.result.unwrap().id, "task-123");
    }

    #[test]
    fn test_a2a_message_with_config() {
        let config = MessageConfiguration {
            streaming: Some(true),
            ..Default::default()
        };

        let msg = A2AMessage::send("Hello").with_config(config);
        let params = msg.params.unwrap();
        assert!(params.configuration.unwrap().streaming.unwrap());
    }

    #[test]
    fn test_request_ids_are_unique() {
        let msg1 = A2AMessage::send("first");
        let msg2 = A2AMessage::send("second");
        let msg3 = A2AMessage::get_task("task-1");
        let msg4 = A2AMessage::cancel_task("task-2");

        // Each message must receive a distinct ID (all pairs, not just adjacent).
        assert_ne!(msg1.id, msg2.id);
        assert_ne!(msg1.id, msg3.id);
        assert_ne!(msg1.id, msg4.id);
        assert_ne!(msg2.id, msg3.id);
        assert_ne!(msg2.id, msg4.id);
        assert_ne!(msg3.id, msg4.id);
    }

    #[test]
    fn test_request_id_is_numeric() {
        let msg = A2AMessage::send("test");
        assert!(
            msg.id.is_number(),
            "request ID must be a JSON number, got: {:?}",
            msg.id
        );
    }

    #[test]
    fn test_with_id_override() {
        let msg = A2AMessage::send("test").with_id(42u64);
        assert_eq!(msg.id, serde_json::json!(42));
    }
}
