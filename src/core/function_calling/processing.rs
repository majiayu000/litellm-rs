//! Function call processing and tool call handling

use super::executor::FunctionCallingHandler;
use super::types::ToolCall;
use crate::core::models::openai::{ChatMessage, MessageContent, MessageRole};
use crate::utils::error::gateway_error::{GatewayError, Result};
use serde_json::Value;
use tracing::{error, warn};

impl FunctionCallingHandler {
    /// Process tool calls in a chat completion request
    pub async fn process_tool_calls(&self, tool_calls: &[ToolCall]) -> Result<Vec<ChatMessage>> {
        let mut tool_responses = Vec::new();

        for tool_call in tool_calls {
            if tool_call.tool_type != "function" {
                warn!("Unsupported tool type: {}", tool_call.tool_type);
                continue;
            }

            let function_name = &tool_call.function.name;

            if let Some(executor) = self.executors.get(function_name) {
                // Parse function arguments
                let arguments: Value = match serde_json::from_str(&tool_call.function.arguments) {
                    Ok(args) => args,
                    Err(e) => {
                        error!("Failed to parse function arguments: {}", e);
                        return Err(GatewayError::Validation(format!(
                            "Invalid function arguments: {}",
                            e
                        )));
                    }
                };

                // Validate arguments
                if let Err(e) = executor.validate_arguments(&arguments) {
                    error!("Function argument validation failed: {}", e);
                    return Err(e);
                }

                // Execute function
                match executor.execute(arguments).await {
                    Ok(result) => {
                        let tool_message = ChatMessage {
                            thinking: None,
                            role: MessageRole::Tool,
                            content: Some(MessageContent::Text(result.to_string())),
                            name: Some(function_name.clone()),
                            function_call: None,
                            tool_calls: None,
                            tool_call_id: None,
                            audio: None,
                        };
                        tool_responses.push(tool_message);
                    }
                    Err(e) => {
                        error!("Function execution failed: {}", e);
                        let error_message = ChatMessage {
                            thinking: None,
                            role: MessageRole::Tool,
                            content: Some(MessageContent::Text(format!("Error: {}", e))),
                            name: Some(function_name.clone()),
                            function_call: None,
                            tool_calls: None,
                            tool_call_id: None,
                            audio: None,
                        };
                        tool_responses.push(error_message);
                    }
                }
            } else {
                warn!("Unknown function: {}", function_name);
                let error_message = ChatMessage {
                    thinking: None,
                    role: MessageRole::Tool,
                    content: Some(MessageContent::Text(format!(
                        "Unknown function: {}",
                        function_name
                    ))),
                    name: Some(function_name.clone()),
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                    audio: None,
                };
                tool_responses.push(error_message);
            }
        }

        Ok(tool_responses)
    }
}
