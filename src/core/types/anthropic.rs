//! Anthropic-specific request types

use super::chat::ChatRequest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Anthropic-specific thinking configuration (legacy)
///
/// Note: For the unified thinking config, use `crate::core::types::thinking::ThinkingConfig`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicThinkingConfig {
    /// Enable thinking mode
    pub enabled: bool,
}

/// Computer tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerToolConfig {
    /// Screen width
    pub display_width: u32,
    /// Screen height
    pub display_height: u32,
    /// Display density
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_density: Option<u32>,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,
    /// Server endpoint
    pub endpoint: String,
    /// Authentication info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<serde_json::Value>,
}

/// Anthropic request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicRequestParams {
    /// System message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Top K sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<AnthropicMetadata>,
    /// Thinking configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<AnthropicThinkingConfig>,
    /// Computer use configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computer_use: Option<ComputerToolConfig>,
    /// MCP server list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<McpServerConfig>>,
}

/// Anthropic metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMetadata {
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Custom data
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// Enhanced ChatRequest to support Anthropic features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicChatRequest {
    #[serde(flatten)]
    pub base: ChatRequest,
    #[serde(flatten)]
    pub anthropic_params: AnthropicRequestParams,
}

/// Anthropic server-side (built-in) tool types.
///
/// These are tools that Anthropic hosts and runs server-side, as opposed to
/// user-defined function tools.  Pass a list of these via the `"anthropic_tools"`
/// key in `ChatRequest::extra_params`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnthropicBuiltinToolType {
    /// Web search tool — searches the web for real-time information.
    #[serde(rename = "web_search_20250305")]
    WebSearch,
    /// Computer use tool — controls a virtual desktop environment.
    #[serde(rename = "computer_20241022")]
    ComputerUse,
}

/// An Anthropic server-side built-in tool definition.
///
/// Populate `display_width_px` / `display_height_px` when using
/// `AnthropicBuiltinToolType::ComputerUse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicBuiltinTool {
    /// The built-in tool type.
    #[serde(rename = "type")]
    pub tool_type: AnthropicBuiltinToolType,
    /// Display width in pixels (required for `computer_20241022`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_width_px: Option<u32>,
    /// Display height in pixels (required for `computer_20241022`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_height_px: Option<u32>,
    /// X display number (optional, for `computer_20241022`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_number: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== AnthropicThinkingConfig Tests ====================

    #[test]
    fn test_thinking_config_enabled() {
        let config = AnthropicThinkingConfig { enabled: true };
        assert!(config.enabled);
    }

    #[test]
    fn test_thinking_config_disabled() {
        let config = AnthropicThinkingConfig { enabled: false };
        assert!(!config.enabled);
    }

    #[test]
    fn test_thinking_config_serialization() {
        let config = AnthropicThinkingConfig { enabled: true };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
    }

    #[test]
    fn test_thinking_config_deserialization() {
        let json = r#"{"enabled": false}"#;
        let config: AnthropicThinkingConfig = serde_json::from_str(json).unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn test_thinking_config_clone() {
        let config = AnthropicThinkingConfig { enabled: true };
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
    }

    // ==================== ComputerToolConfig Tests ====================

    #[test]
    fn test_computer_tool_config_structure() {
        let config = ComputerToolConfig {
            display_width: 1920,
            display_height: 1080,
            display_density: None,
        };
        assert_eq!(config.display_width, 1920);
        assert_eq!(config.display_height, 1080);
    }

    #[test]
    fn test_computer_tool_config_with_density() {
        let config = ComputerToolConfig {
            display_width: 2560,
            display_height: 1440,
            display_density: Some(2),
        };
        assert_eq!(config.display_density, Some(2));
    }

    #[test]
    fn test_computer_tool_config_serialization() {
        let config = ComputerToolConfig {
            display_width: 1280,
            display_height: 720,
            display_density: Some(1),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["display_width"], 1280);
        assert_eq!(json["display_height"], 720);
        assert_eq!(json["display_density"], 1);
    }

    #[test]
    fn test_computer_tool_config_skip_none_density() {
        let config = ComputerToolConfig {
            display_width: 800,
            display_height: 600,
            display_density: None,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert!(!json.as_object().unwrap().contains_key("display_density"));
    }

    #[test]
    fn test_computer_tool_config_clone() {
        let config = ComputerToolConfig {
            display_width: 1920,
            display_height: 1080,
            display_density: Some(2),
        };
        let cloned = config.clone();
        assert_eq!(config.display_width, cloned.display_width);
    }

    // ==================== McpServerConfig Tests ====================

    #[test]
    fn test_mcp_server_config_structure() {
        let config = McpServerConfig {
            name: "test-server".to_string(),
            endpoint: "https://mcp.example.com".to_string(),
            auth: None,
        };
        assert_eq!(config.name, "test-server");
        assert_eq!(config.endpoint, "https://mcp.example.com");
    }

    #[test]
    fn test_mcp_server_config_with_auth() {
        let config = McpServerConfig {
            name: "secure-server".to_string(),
            endpoint: "https://secure.example.com".to_string(),
            auth: Some(serde_json::json!({"token": "secret"})),
        };
        assert!(config.auth.is_some());
    }

    #[test]
    fn test_mcp_server_config_serialization() {
        let config = McpServerConfig {
            name: "server".to_string(),
            endpoint: "https://api.example.com".to_string(),
            auth: Some(serde_json::json!({"type": "bearer", "token": "abc"})),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["name"], "server");
        assert_eq!(json["auth"]["type"], "bearer");
    }

    #[test]
    fn test_mcp_server_config_clone() {
        let config = McpServerConfig {
            name: "clone-test".to_string(),
            endpoint: "https://test.com".to_string(),
            auth: None,
        };
        let cloned = config.clone();
        assert_eq!(config.name, cloned.name);
    }

    // ==================== AnthropicMetadata Tests ====================

    #[test]
    fn test_anthropic_metadata_empty() {
        let metadata = AnthropicMetadata {
            user_id: None,
            session_id: None,
            custom: HashMap::new(),
        };
        assert!(metadata.user_id.is_none());
        assert!(metadata.custom.is_empty());
    }

    #[test]
    fn test_anthropic_metadata_with_ids() {
        let metadata = AnthropicMetadata {
            user_id: Some("user-123".to_string()),
            session_id: Some("session-456".to_string()),
            custom: HashMap::new(),
        };
        assert_eq!(metadata.user_id, Some("user-123".to_string()));
        assert_eq!(metadata.session_id, Some("session-456".to_string()));
    }

    #[test]
    fn test_anthropic_metadata_with_custom() {
        let mut custom = HashMap::new();
        custom.insert("key".to_string(), serde_json::json!("value"));

        let metadata = AnthropicMetadata {
            user_id: None,
            session_id: None,
            custom,
        };
        assert_eq!(
            metadata.custom.get("key"),
            Some(&serde_json::json!("value"))
        );
    }

    #[test]
    fn test_anthropic_metadata_serialization() {
        let metadata = AnthropicMetadata {
            user_id: Some("user".to_string()),
            session_id: None,
            custom: HashMap::new(),
        };
        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["user_id"], "user");
    }

    #[test]
    fn test_anthropic_metadata_clone() {
        let metadata = AnthropicMetadata {
            user_id: Some("user".to_string()),
            session_id: None,
            custom: HashMap::new(),
        };
        let cloned = metadata.clone();
        assert_eq!(metadata.user_id, cloned.user_id);
    }

    // ==================== AnthropicRequestParams Tests ====================

    #[test]
    fn test_request_params_empty() {
        let params = AnthropicRequestParams {
            system: None,
            stop_sequences: None,
            top_k: None,
            metadata: None,
            thinking: None,
            computer_use: None,
            mcp_servers: None,
        };
        assert!(params.system.is_none());
        assert!(params.stop_sequences.is_none());
    }

    #[test]
    fn test_request_params_with_system() {
        let params = AnthropicRequestParams {
            system: Some("You are a helpful assistant.".to_string()),
            stop_sequences: None,
            top_k: None,
            metadata: None,
            thinking: None,
            computer_use: None,
            mcp_servers: None,
        };
        assert_eq!(
            params.system,
            Some("You are a helpful assistant.".to_string())
        );
    }

    #[test]
    fn test_request_params_with_stop_sequences() {
        let params = AnthropicRequestParams {
            system: None,
            stop_sequences: Some(vec!["STOP".to_string(), "END".to_string()]),
            top_k: Some(40),
            metadata: None,
            thinking: None,
            computer_use: None,
            mcp_servers: None,
        };
        assert_eq!(params.stop_sequences.as_ref().unwrap().len(), 2);
        assert_eq!(params.top_k, Some(40));
    }

    #[test]
    fn test_request_params_with_thinking() {
        let params = AnthropicRequestParams {
            system: None,
            stop_sequences: None,
            top_k: None,
            metadata: None,
            thinking: Some(AnthropicThinkingConfig { enabled: true }),
            computer_use: None,
            mcp_servers: None,
        };
        assert!(params.thinking.as_ref().unwrap().enabled);
    }

    #[test]
    fn test_request_params_serialization() {
        let params = AnthropicRequestParams {
            system: Some("System prompt".to_string()),
            stop_sequences: Some(vec!["STOP".to_string()]),
            top_k: Some(50),
            metadata: None,
            thinking: None,
            computer_use: None,
            mcp_servers: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["system"], "System prompt");
        assert_eq!(json["top_k"], 50);
    }

    #[test]
    fn test_request_params_clone() {
        let params = AnthropicRequestParams {
            system: Some("test".to_string()),
            stop_sequences: None,
            top_k: None,
            metadata: None,
            thinking: None,
            computer_use: None,
            mcp_servers: None,
        };
        let cloned = params.clone();
        assert_eq!(params.system, cloned.system);
    }

    // ==================== AnthropicBuiltinTool Tests ====================

    #[test]
    fn test_builtin_tool_type_web_search_serialization() {
        let t = AnthropicBuiltinToolType::WebSearch;
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json, "web_search_20250305");
    }

    #[test]
    fn test_builtin_tool_type_computer_use_serialization() {
        let t = AnthropicBuiltinToolType::ComputerUse;
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json, "computer_20241022");
    }

    #[test]
    fn test_builtin_tool_web_search_round_trip() {
        let tool = AnthropicBuiltinTool {
            tool_type: AnthropicBuiltinToolType::WebSearch,
            display_width_px: None,
            display_height_px: None,
            display_number: None,
        };
        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["type"], "web_search_20250305");
        assert!(!json.as_object().unwrap().contains_key("display_width_px"));
    }

    #[test]
    fn test_builtin_tool_computer_use_round_trip() {
        let tool = AnthropicBuiltinTool {
            tool_type: AnthropicBuiltinToolType::ComputerUse,
            display_width_px: Some(1920),
            display_height_px: Some(1080),
            display_number: Some(1),
        };
        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["type"], "computer_20241022");
        assert_eq!(json["display_width_px"], 1920);
        assert_eq!(json["display_height_px"], 1080);
        assert_eq!(json["display_number"], 1);
    }

    #[test]
    fn test_builtin_tool_deserialization() {
        let json = r#"{"type":"web_search_20250305"}"#;
        let tool: AnthropicBuiltinTool = serde_json::from_str(json).unwrap();
        assert_eq!(tool.tool_type, AnthropicBuiltinToolType::WebSearch);
        assert!(tool.display_width_px.is_none());
    }
}
