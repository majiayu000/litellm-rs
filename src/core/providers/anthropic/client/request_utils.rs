use std::collections::{HashMap, hash_map::Entry};

use serde_json::{Value, json};

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use crate::core::types::content::ContentPart;
use crate::core::types::tools::Tool;

use super::AnthropicClient;

const ANTHROPIC_TOOL_NAME_MAX_LEN: usize = 64;

pub(super) type ToolNameMap = HashMap<String, String>;

pub(super) fn anthropic_tool_name(name: &str) -> String {
    let mut sanitized = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized.push_str("tool");
    }
    sanitized.truncate(ANTHROPIC_TOOL_NAME_MAX_LEN);
    sanitized
}

pub(super) fn anthropic_tools(tools: &[Tool]) -> Result<Vec<Value>, ProviderError> {
    anthropic_tool_name_map(tools)?;
    Ok(tools
        .iter()
        .map(|tool| {
            json!({
                "name": anthropic_tool_name(&tool.function.name),
                "description": tool.function.description.as_deref().unwrap_or(""),
                "input_schema": tool.function.parameters.as_ref().unwrap_or(&json!({}))
            })
        })
        .collect())
}

pub(super) fn anthropic_tool_name_map(tools: &[Tool]) -> Result<ToolNameMap, ProviderError> {
    let mut names = HashMap::new();

    for tool in tools {
        let name = &tool.function.name;
        let sanitized = anthropic_tool_name(name);
        match names.entry(sanitized.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(name.clone());
            }
            Entry::Occupied(entry) => {
                return Err(ProviderError::invalid_request(
                    "anthropic",
                    format!(
                        "Tool name '{}' collides with '{}' after Anthropic name sanitization to '{}'",
                        name,
                        entry.get(),
                        sanitized
                    ),
                ));
            }
        }
    }

    Ok(names)
}

pub(super) fn restore_tool_name(name: &str, map: &ToolNameMap) -> String {
    map.get(name).cloned().unwrap_or_else(|| name.to_string())
}

pub(super) fn declared_tool_name(
    name: &str,
    map: &ToolNameMap,
    field: &str,
) -> Result<String, ProviderError> {
    let sanitized = anthropic_tool_name(name);
    if map.is_empty() {
        return Ok(sanitized);
    }

    match map.get(&sanitized) {
        Some(original) if original == name => Ok(sanitized),
        Some(original) => Err(ProviderError::invalid_request(
            "anthropic",
            format!(
                "{} '{}' sanitizes to '{}' but declared tool '{}' already uses that Anthropic name",
                field, name, sanitized, original
            ),
        )),
        None => Err(ProviderError::invalid_request(
            "anthropic",
            format!("{} '{}' does not match a declared tool", field, name),
        )),
    }
}

pub(super) fn unsupported_content_part(part: &ContentPart) -> ProviderError {
    ProviderError::invalid_request(
        "anthropic",
        format!(
            "Anthropic request transformation does not support {} content parts in this position",
            content_part_type(part)
        ),
    )
}

fn content_part_type(part: &ContentPart) -> &'static str {
    match part {
        ContentPart::Text { .. } => "text",
        ContentPart::ImageUrl { .. } => "image_url",
        ContentPart::Audio { .. } => "audio",
        ContentPart::Image { .. } => "image",
        ContentPart::Document { .. } => "document",
        ContentPart::ToolResult { .. } => "tool_result",
        ContentPart::ToolUse { .. } => "tool_use",
    }
}

impl AnthropicClient {
    pub(crate) fn anthropic_tool_name_map_for_request(
        &self,
        request: &ChatRequest,
    ) -> Result<ToolNameMap, ProviderError> {
        request
            .tools
            .as_deref()
            .map(anthropic_tool_name_map)
            .unwrap_or_else(|| Ok(HashMap::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_761_sanitizes_tool_names_to_anthropic_shape() {
        assert_eq!(
            anthropic_tool_name("get.weather forecast"),
            "get_weather_forecast"
        );
        assert_eq!(anthropic_tool_name(""), "tool");
        assert_eq!(anthropic_tool_name(&"a".repeat(80)).len(), 64);
    }

    #[test]
    fn issue_761_rejects_sanitized_tool_name_collisions() {
        let tools = vec![test_tool("get.weather"), test_tool("get_weather")];
        let error =
            anthropic_tool_name_map(&tools).expect_err("sanitized tool names must be unique");
        let message = error.to_string();

        assert!(message.contains("get.weather"));
        assert!(message.contains("get_weather"));
    }

    fn test_tool(name: &str) -> Tool {
        Tool {
            tool_type: crate::core::types::tools::ToolType::Function,
            function: crate::core::types::tools::FunctionDefinition {
                name: name.to_string(),
                description: None,
                parameters: None,
            },
        }
    }
}
