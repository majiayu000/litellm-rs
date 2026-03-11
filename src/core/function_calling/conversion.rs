//! Provider-specific format conversion for function calling

use super::executor::FunctionCallingHandler;
use super::types::{FunctionCall, ToolCall, ToolDefinition};
use crate::utils::error::gateway_error::{GatewayError, Result};
use serde_json::{Value, json};

impl FunctionCallingHandler {
    /// Convert function definitions to provider-specific format
    pub fn convert_tools_for_provider(
        &self,
        provider_type: &crate::core::providers::ProviderType,
        tools: &[ToolDefinition],
    ) -> Result<Value> {
        match provider_type {
            crate::core::providers::ProviderType::OpenAI
            | crate::core::providers::ProviderType::Azure => {
                // OpenAI format
                Ok(json!(tools))
            }
            crate::core::providers::ProviderType::Anthropic => {
                // Anthropic format
                let anthropic_tools: Vec<Value> = tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "name": tool.function.name,
                            "description": tool.function.description,
                            "input_schema": tool.function.parameters
                        })
                    })
                    .collect();
                Ok(json!(anthropic_tools))
            }
            crate::core::providers::ProviderType::VertexAI => {
                // Google VertexAI format
                let google_tools: Vec<Value> = tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "function_declarations": [{
                                "name": tool.function.name,
                                "description": tool.function.description,
                                "parameters": tool.function.parameters
                            }]
                        })
                    })
                    .collect();
                Ok(json!(google_tools))
            }
            _ => Err(GatewayError::bad_request(format!(
                "Function calling not supported for provider: {:?}",
                provider_type
            ))),
        }
    }

    /// Extract tool calls from provider response
    pub fn extract_tool_calls_from_response(
        &self,
        provider_type: &crate::core::providers::ProviderType,
        response: &Value,
    ) -> Result<Vec<ToolCall>> {
        match provider_type {
            crate::core::providers::ProviderType::OpenAI
            | crate::core::providers::ProviderType::Azure => {
                // OpenAI format
                if let Some(choices) = response.get("choices").and_then(|c| c.as_array())
                    && let Some(choice) = choices.first()
                    && let Some(message) = choice.get("message")
                    && let Some(tool_calls) = message.get("tool_calls")
                {
                    let tool_calls: Vec<ToolCall> = serde_json::from_value(tool_calls.clone())?;
                    return Ok(tool_calls);
                }
                Ok(vec![])
            }
            crate::core::providers::ProviderType::Anthropic => {
                // Anthropic format
                if let Some(content) = response.get("content").and_then(|c| c.as_array()) {
                    let mut tool_calls = Vec::new();
                    for item in content {
                        if let Some(tool_type) = item.get("type").and_then(|t| t.as_str())
                            && tool_type == "tool_use"
                            && let (Some(id), Some(name), Some(input)) = (
                                item.get("id").and_then(|i| i.as_str()),
                                item.get("name").and_then(|n| n.as_str()),
                                item.get("input"),
                            )
                        {
                            tool_calls.push(ToolCall {
                                id: id.to_string(),
                                tool_type: "function".to_string(),
                                function: FunctionCall {
                                    name: name.to_string(),
                                    arguments: input.to_string(),
                                },
                            });
                        }
                    }
                    return Ok(tool_calls);
                }
                Ok(vec![])
            }
            _ => Ok(vec![]),
        }
    }
}
