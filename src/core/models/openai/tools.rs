//! Tool and function calling types for OpenAI-compatible API
//!
//! This module defines structures for function calling (legacy) and tool calling,
//! including function definitions, tool choices, and tool calls.

use serde::{Deserialize, Serialize};

/// Function definition (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Function description
    pub description: Option<String>,
    /// Function parameters schema
    pub parameters: Option<serde_json::Value>,
}

/// Function call (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function name
    pub name: String,
    /// Function arguments (JSON string)
    pub arguments: String,
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
    /// No tool calls allowed
    None(String), // "none"
    /// Automatic tool selection
    Auto(String), // "auto"
    /// Tool calls required
    Required(String), // "required"
    /// Specific tool to use
    Specific(ToolChoiceFunction),
}

/// Specific tool choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    /// Tool type
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function specification
    pub function: ToolChoiceFunctionSpec,
}

/// Tool choice function specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunctionSpec {
    /// Function name
    pub name: String,
}

/// Tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,
    /// Tool type
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function call
    pub function: FunctionCall,
}

impl From<FunctionCall> for crate::core::types::tools::FunctionCall {
    fn from(value: FunctionCall) -> Self {
        Self {
            name: value.name,
            arguments: value.arguments,
        }
    }
}

impl From<crate::core::types::tools::FunctionCall> for FunctionCall {
    fn from(value: crate::core::types::tools::FunctionCall) -> Self {
        Self {
            name: value.name,
            arguments: value.arguments,
        }
    }
}

impl From<ToolCall> for crate::core::types::tools::ToolCall {
    fn from(value: ToolCall) -> Self {
        Self {
            id: value.id,
            tool_type: value.tool_type,
            function: value.function.into(),
        }
    }
}

impl From<crate::core::types::tools::ToolCall> for ToolCall {
    fn from(value: crate::core::types::tools::ToolCall) -> Self {
        Self {
            id: value.id,
            tool_type: value.tool_type,
            function: value.function.into(),
        }
    }
}

// FunctionCallDelta and ToolCallDelta are re-exported from the canonical
// types tree (core::types::responses::*). The duplicates that lived here
// had field-compatible layouts and were a downstream of the
// parallel-type-tree schism (#519, audit A-2/A-5).
pub use crate::core::types::responses::{FunctionCallDelta, ToolCallDelta};
