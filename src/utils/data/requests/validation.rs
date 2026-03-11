use crate::core::providers::unified_provider::ProviderError;
use serde_json::Value;

use super::types::{MessageContent, RequestUtils};

impl RequestUtils {
    pub fn validate_chat_completion_messages(
        messages: &[MessageContent],
    ) -> Result<(), ProviderError> {
        if messages.is_empty() {
            return Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: "Messages array cannot be empty".to_string(),
            });
        }

        for (i, message) in messages.iter().enumerate() {
            Self::validate_single_message(message, i)?;
        }

        Self::validate_message_sequence(messages)?;
        Ok(())
    }

    fn validate_single_message(
        message: &MessageContent,
        index: usize,
    ) -> Result<(), ProviderError> {
        let valid_roles = ["system", "user", "assistant", "function", "tool"];

        if !valid_roles.contains(&message.role.as_str()) {
            return Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: format!("Invalid role '{}' at message index {}", message.role, index),
            });
        }

        if message.content.is_empty() && message.role != "tool" {
            return Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: format!("Message content cannot be empty at index {}", index),
            });
        }

        if message.content.len() > 100000 {
            return Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: format!(
                    "Message content too long at index {} (max 100k chars)",
                    index
                ),
            });
        }

        Ok(())
    }

    fn validate_message_sequence(messages: &[MessageContent]) -> Result<(), ProviderError> {
        let mut has_user_message = false;

        for message in messages {
            if message.role == "user" {
                has_user_message = true;
                break;
            }
        }

        if !has_user_message {
            return Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: "At least one user message is required".to_string(),
            });
        }

        Ok(())
    }

    pub fn validate_and_fix_openai_messages(
        messages: &mut [MessageContent],
    ) -> Result<(), ProviderError> {
        for message in messages.iter_mut() {
            Self::cleanup_none_fields_in_message(message);
        }

        Self::validate_chat_completion_messages(messages)?;
        Ok(())
    }

    fn cleanup_none_fields_in_message(message: &mut MessageContent) {
        message.content = message.content.trim().to_string();
    }

    pub fn validate_and_fix_openai_tools(
        tools: &mut Option<Vec<Value>>,
    ) -> Result<(), ProviderError> {
        if let Some(tools_vec) = tools {
            for (i, tool) in tools_vec.iter().enumerate() {
                if !tool.is_object() {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!("Tool at index {} must be an object", i),
                    });
                }

                let Some(tool_obj) = tool.as_object() else {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!("Tool at index {} must be an object", i),
                    });
                };

                if !tool_obj.contains_key("type") {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!("Tool at index {} missing required 'type' field", i),
                    });
                }

                if !tool_obj.contains_key("function") {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!("Tool at index {} missing required 'function' field", i),
                    });
                }

                let Some(function) = tool_obj.get("function") else {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!("Tool at index {} missing required 'function' field", i),
                    });
                };
                if !function.is_object() {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!("Tool function at index {} must be an object", i),
                    });
                }

                let Some(func_obj) = function.as_object() else {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!("Tool function at index {} must be an object", i),
                    });
                };
                if !func_obj.contains_key("name") {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: format!(
                            "Tool function at index {} missing required 'name' field",
                            i
                        ),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn validate_tool_choice(
        tool_choice: &Option<String>,
        tools: &Option<Vec<Value>>,
    ) -> Result<(), ProviderError> {
        if let Some(choice) = tool_choice {
            let Some(tools_vec) = tools.as_ref() else {
                return Err(ProviderError::InvalidRequest {
                    provider: "unknown",
                    message: "tool_choice requires tools to be provided".to_string(),
                });
            };

            if tools_vec.is_empty() {
                return Err(ProviderError::InvalidRequest {
                    provider: "unknown",
                    message: "tool_choice requires tools to be provided".to_string(),
                });
            }

            match choice.as_str() {
                "none" | "auto" => {}
                _ => {
                    if !Self::is_valid_tool_name(choice, tools_vec) {
                        return Err(ProviderError::InvalidRequest {
                            provider: "unknown",
                            message: format!(
                                "Invalid tool_choice '{}'. Must be 'none', 'auto', or a valid tool name",
                                choice
                            ),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn is_valid_tool_name(tool_name: &str, tools: &[Value]) -> bool {
        tools.iter().any(|tool| {
            if let Some(function) = tool.get("function")
                && let Some(name) = function.get("name")
            {
                return name.as_str() == Some(tool_name);
            }
            false
        })
    }
}
