use serde::{Deserialize, Serialize};

/// Tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Call ID
    pub id: String,
    /// Tool type
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function call
    pub function: Function,
}

/// Function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Function description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Function parameter schema
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub parameters: serde_json::Value,
    /// Function parameters (used for calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Tool type
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function definition
    pub function: Function,
}

/// Tool choice
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Don't use tools
    None,
    /// Auto selection
    Auto,
    /// Must use tools
    Required,
    /// Specific function
    Function {
        /// Function name
        name: String,
    },
}
